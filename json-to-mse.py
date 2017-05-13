#!/usr/bin/env python3

import sys

import contextlib
import enum
import io
import more_itertools
import mtgjson
import re
import zipfile

CACHE = {
    'db': None,
    'db_x': None
}

BASIC_LAND_TYPES = {
    'Plains': 'W',
    'Island': 'U',
    'Swamp': 'B',
    'Mountain': 'R',
    'Forest': 'G'
}

COLOR_ABBREVIATIONS = {
    'W': 'White',
    'U': 'Blue',
    'B': 'Black',
    'R': 'Red',
    'G': 'Green'
}

class CommandLineArgs:
    def __init__(self, args=sys.argv[1:]):
        self.verbose = False
        self.cards = set()
        self.output = sys.stdout.buffer
        self.border_color = None
        self.copyright = 'NOT FOR SALE'
        self.old_wedge_order = False
        self.set_code = 'PROXY'
        mode = None
        for arg in args:
            if mode == 'border':
                self.set_border_color(arg)
                mode = None
            elif mode == 'copyright':
                self.copyright = arg
                mode = None
            elif mode == 'input':
                self.set_input(arg)
                mode = None
            elif mode == 'output':
                self.output = open(arg, 'wb')
                mode = None
            elif mode == 'set-code':
                self.set_code = arg
                mode = None
            elif arg.startswith('-'):
                if arg.startswith('--'):
                    if arg == '--border':
                        mode = 'border'
                    elif arg.startswith('--border='):
                        self.set_border_color(arg[len('--border='):])
                    elif arg == '--copyright':
                        mode = 'copyright'
                    elif arg.startswith('--copyright='):
                        self.copyright = arg[len('--copyright='):]
                    elif arg == '--input':
                        mode = 'input'
                    elif arg.startswith('--input='):
                        self.set_input(arg[len('--input='):])
                    elif arg == '--old-wedge-order':
                        self.old_wedge_order = True
                    elif arg == '--output':
                        mode = 'output'
                    elif arg.startswith('--output='):
                        self.output = open(arg[len('--output='):], 'wb')
                    elif arg == '--set-code':
                        mode = 'set-code'
                    elif arg.startswith('--set-code='):
                        self.set_code = arg[len('--set-code='):]
                    elif arg == '--verbose':
                        self.verbose = True
                    else:
                        raise ValueError(f'Unrecognized flag: {arg}')
                else:
                    for i, short_flag in enumerate(arg):
                        if i == 0:
                            continue
                        if short_flag == 'b':
                            if len(arg) > i + 1:
                                self.set_border_color(arg[i + 1:])
                            else:
                                mode = 'border'
                            break
                        elif short_flag == 'i':
                            if len(arg) > i + 1:
                                self.set_input(arg[i + 1:])
                            else:
                                mode = 'input'
                            break
                        elif short_flag == 'o':
                            if len(arg) > i + 1:
                                self.output = open(arg[i + 1:], 'wb')
                            else:
                                mode = 'output'
                            break
                        elif short_flag == 'v':
                            self.verbose = True
                        else:
                            raise ValueError(f'Unrecognized flag: -{short_flag}')
            else:
                self.cards.add(arg)

    def set_border_color(self, border_color):
        if border_color == 'black':
            self.border_color = None
        elif border_color in ('w', 'white'):
            self.border_color = 'rgb(255,255,255)'
        elif border_color in ('s', 'silver'):
            self.border_color = 'rgb(128,128,128)'
        elif border_color in ('g', 'gold'):
            self.border_color = 'rgb(200,180,0)'
        elif border_color in ('b', 'bronze'):
            self.border_color = 'rgb(222,127,50)'
        else:
            raise ValueError(f'Unrecognized border color: {border_color}')

    def set_input(self, input_filename):
        with open(input_filename) as f:
            for line in f:
                if line.strip() == '':
                    continue
                if line.strip().startswith('#'):
                    continue
                self.cards.add(line.strip())

class FrameFeatures(enum.Flag):
    NONE = 0
    AFTERMATH = enum.auto()
    DEVOID = enum.auto()
    DFC = enum.auto()
    FUSE = enum.auto()
    MIRACLE = enum.auto()
    NYX = enum.auto()
    PLANESWALKER = enum.auto()
    PLANESWALKER_BACK = enum.auto()
    SPLIT = enum.auto()

    def alt_dfc(self):
        try:
            return {
                FrameFeatures.NONE: FrameFeatures.NONE,
                FrameFeatures.PLANESWALKER: FrameFeatures.PLANESWALKER_BACK
            }[self]
        except KeyError as e:
            raise NotImplementedError('Frame features {} not implemented for DFC back faces'.format(self.name)) from e

class MSEDataFile:
    def __init__(self, data={}):
        self.items = []
        for key, value in data.items():
            self[key] = value

    def __contains__(self, key):
        for iter_key, value in self.items:
            if iter_key == key:
                return True
        else:
            return False

    def __getitem__(self, key):
        result = None
        for iter_key, value in self.items:
            if iter_key == key:
                if result is None:
                    result = value
                else:
                    raise KeyError(f'Multiple values for key {key!r}')
        if result is None:
            raise KeyError(f'No values for key {key!r}')
        else:
            return result

    def __ior__(self, other):
        for key, value in other.items:
            self[key] = value
        return self

    def __setitem__(self, key, value):
        for iter_key, iter_value in self.items:
            if iter_key == key:
                raise KeyError('Key exists')
        self.add(key, value)

    def __str__(self):
        return self.to_string()

    def add_card(self, card_info, db):
        card = self.__class__.from_card(card_info, db)
        self.add('card', card)
        with contextlib.suppress(KeyError):
            stylesheet = card['stylesheet']
            if not hasattr(self, 'stylesheets'):
                self.stylesheets = set()
            self.stylesheets.add(stylesheet)

    @classmethod
    def from_card(cls, card_info, db, *, alt=False):
        def alt_key(key_name):
            if alt:
                return f'{key_name} {alt}'
            else:
                return key_name

        raw_data = card_info._get_raw_data()
        # check for legality
        if raw_data.get('border', card_info.set.border) == 'silver':
            raise ValueError('Un-cards are not supported')
        if card_info.layout == 'token':
            raise ValueError('Token cards are not supported')
        if card_info.name in ['1996 World Champion', 'Fraternal Exaltation', 'Proposal', 'Robot Chicken', 'Shichifukujin Dragon', 'Splendid Genesis']:
            raise ValueError('This card is blacklisted and will not be supported')
        # collect printings
        printings = {}
        for set_code, set_info in db.sets.items():
            for iter_card in set_info.cards_by_name.values():
                if iter_card.name == card_info.name:
                    printings[set_code] = iter_card
        result = cls()
        frame_features = FrameFeatures.NONE
        # layout
        if card_info.layout == 'normal':
            pass # nothing specific to normal layout
        elif card_info.layout == 'split':
            if not alt:
                frame_features |= FrameFeatures.SPLIT
                alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, alt=2)
                result |= alt_result
                frame_features |= alt_frame_features
        elif card_info.layout == 'double-faced':
            if not alt:
                frame_features |= FrameFeatures.DFC
                alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, alt=2)
                result |= alt_result
                frame_features |= alt_frame_features.alt_dfc()
        else:
            raise NotImplementedError(f'Unsupported layout: {card_info.layout}') #TODO flip, double-faced, plane, scheme, phenomenon, leveler, vanguard, meld, aftermath
        # name
        result[alt_key('name')] = card_info.name
        # mana cost
        if 'manaCost' in raw_data:
            result[alt_key('casting cost')] = cost_to_mse(card_info.manaCost)
            #TODO check to add hybrid frame feature
        # color indicator
        if set(raw_data.get('colors', [])) != set(implicit_colors(raw_data.get('manaCost'))):
            if raw_data.get('colors', []) == []:
                frame_features |= FrameFeature.DEVOID
            else:
                result[alt_key('card color')] = result[alt_key('indicator')] = ', '.join(c.lower() for c in card_info.colors)
                #TODO make sure MSE renders two-color gold cards in the correct order
                #TODO make sure MSE renders 3+ color gold cards without the gradient
        # type line
        if 'supertypes' in raw_data:
            result[alt_key('super type')] = f'<word-list-type>{" ".join(card_info.supertypes)} {" ".join(card_info.types)}</word-list-type>'
        else:
            result[alt_key('super type')] = f'<word-list-type>{" ".join(card_info.types)}</word-list-type>'
        if 'subtypes' in raw_data:
            if 'Creature' in card_info.types:
                card_type = 'race'
            elif 'Instant' in card_info.types or 'Sorcery' in card_info.types:
                card_type = 'spell'
            else:
                card_type = card_info.types[0].lower()
            result[alt_key('sub type')] = ' '.join(f'<word-list-{card_type}>{subtype}</word-list-race>' for subtype in card_info.subtypes)
        if 'Planeswalker' in card_info.types:
            frame_features |= FrameFeatures.PLANESWALKER
        if 'Enchantment' in card_info.types and more_itertools.ilen(card_type for card_type in card_info.types if card_type != 'Tribal') > 2:
            frame_features |= FrameFeatures.NYX
        # rarity
        result[alt_key('rarity')] = min(Rarity.from_str(printing.rarity) for printing in printings.values()).mse_str
        # text
        if 'text' in raw_data:
            text = ''
            for i, ability in enumerate(card_info.text.split('\n')):
                ability = re.sub(' ?\\([^)]+\\)', '', ability)
                if ability == '':
                    continue
                elif ability == 'Fuse':
                    frame_features |= FrameFeatures.FUSE
                    continue
                match = re.fullmatch('(\\+[0-9]+|-[0-9]+|\u2212[0-9]+|0): (.*)', ability)
                if 'Planeswalker' in card_info.type and match:
                    result[f'loyalty cost {4 * alt + i + 1}'] = match.group(1).replace('\u2212', '-')
                    ability = match.group(2)
                if text != '':
                    text += '\n'
                for j, word in enumerate(ability.split(' ')):
                    if j > 0:
                        text += ' '
                    if j == 0 and word == 'Miracle':
                        frame_features |= FrameFeatures.MIRACLE
                    match = re.fullmatch('("?)(\\{.+\\})([:.,]?)', word)
                    if match:
                        text += f'{match.group(1)}<sym>{cost_to_mse(match.group(2))}</sym>{match.group(3)}'
                    elif re.fullmatch('[0-9]+', word):
                        text += f'</sym>{word}<sym>'
                    else:
                        text += word
        result[alt_key('rule text')] = text
        # mana symbol
        if result[alt_key('rule text')] == '':
            if 'subtypes' in raw_data and more_itertools.quantify(subtype in BASIC_LAND_TYPES for subtype in card_info.subtypes) == 1:
                subtype = more_itertools.one(subtype for subtype in card_info.subtypes if subtype in BASIC_LAND_TYPES)
                result[alt_key('watermark')] = COLOR_ABBREVIATIONS[BASIC_LAND_TYPES[subtype]].lower()
        # P/T
        if 'power' in raw_data:
            result[alt_key('power')] = card_info.power
        if 'toughness' in raw_data:
            result[alt_key('toughness')] = card_info.toughness
        # loyalty
        if 'loyalty' in raw_data:
            result[alt_key('loyalty')] = card_info.loyalty
        # stylesheet
        if alt:
            return result, frame_features
        else:
            if FrameFeatures.SPLIT in frame_features:
                if FrameFeatures.FUSE in frame_features:
                    result['stylesheet'] = 'm15-split-fuse'
                    result['rule text 3'] = 'Fuse' #TODO reminder text based on options
                elif FrameFeatures.AFTERMATH in frame_features:
                    raise NotImplementedError('Aftermath not implemented') #TODO
                else:
                    result['stylesheet'] = 'm15-split'
            elif FrameFeatures.DFC in frame_features:
                if FrameFeatures.PLANESWALKER in frame_features:
                    if FrameFeatures.PLANESWALKER_BACK in frame_features:
                        result['stylesheet'] = 'm15-doublefaced-planeswalker' #TODO borderable?
                    else:
                        raise NotImplementedError('Sacrificer DFCs not implemented')
                else:
                    if FrameFeatures.PLANESWALKER_BACK in frame_features:
                        result['stylesheet'] = 'm15-doublefaced-sparker' #TODO borderable?
                    else:
                        result['stylesheet'] = 'm15-doublefaced'
            elif FrameFeatures.PLANESWALKER in frame_features:
                result['stylesheet'] = 'm15-planeswalker'
            elif FrameFeatures.MIRACLE in frame_features:
                result['stylesheet'] = 'm15-miracle'
            elif FrameFeatures.DEVOID in frame_features:
                result['stylesheet'] = 'm15-devoid'
            elif FrameFeatures.NYX in frame_features:
                result['stylesheet'] = 'm15-nyx'
            return result

    def add(self, key, value):
        if isinstance(value, dict):
            value = MSEDataFile(value)
        self.items.append((key, value))

    def get(self, key):
        for iter_key, value in self.items:
            if iter_key == key:
                yield value

    def to_string(self, indent=0):
        result = ''
        for key, value in self.items:
            result += '\t' * indent
            if isinstance(value, MSEDataFile):
                result += f'{key}:\r\n'
                result += value.to_string(indent=indent + 1)
            else:
                value_str = str(value)
                if '\n' in value_str:
                    result += f'{key}:\r\n'
                    for line in value_str.split('\n'):
                        result += '\t' * (indent + 1)
                        result += f'{line}\r\n'
                else:
                    result += f'{key}: {value}\r\n'
        return result

class OrderedEnum(enum.Enum):
    def __ge__(self, other):
        if self.__class__ is other.__class__:
            return self.value >= other.value
        return NotImplemented

    def __gt__(self, other):
        if self.__class__ is other.__class__:
            return self.value > other.value
        return NotImplemented

    def __le__(self, other):
        if self.__class__ is other.__class__:
            return self.value <= other.value
        return NotImplemented

    def __lt__(self, other):
        if self.__class__ is other.__class__:
            return self.value < other.value
        return NotImplemented

class Rarity(OrderedEnum):
    BASIC = (0, 'basic land')
    COMMON = (1, 'common')
    UNCOMMON = (2, 'uncommon')
    RARE = (3, 'rare')
    MYTHIC = (4, 'mythic rare')
    SPECIAL = (5, 'special')

    def __init__(self, idx, mse_str):
        self.mse_str = mse_str

    @classmethod
    def from_str(cls, rarity_str):
        return {
            'Basic Land': cls.BASIC,
            'Common': cls.COMMON,
            'Uncommon': cls.UNCOMMON,
            'Rare': cls.RARE,
            'Mythic Rare': cls.MYTHIC,
            'Special': cls.SPECIAL
        }[rarity_str]

def cost_to_mse(cost):
    def cost_part_to_mse(part):
        basics = '[WUBRG]'
        if re.fullmatch(basics, part):
            # colored mana
            return part
        if part in ('C', 'Q', 'S', 'T', 'X'):
            # colorless mana, untap symbol, snow mana, tap symbol, variable mana
            return part
        if re.fullmatch('[0-9]+', part):
            # colorless mana
            return part
        if re.fullmatch('{}/{}'.format(basics, basics), part):
            # colored/colored hybrid mana
            return part
        match = re.fullmatch('({})/P'.format(basics), part)
        if match:
            # Phyrexian mana
            return f'H/{match.group(1)}'
        if re.fullmatch('2/{}'.format(basics), part):
            # colorless/colored hybrid mana
            return part
        raise ValueError('Unknown mana cost part: {{{}}}'.format(part))

    if cost is None or cost == '':
        return ''
    if cost[0] != '{' or cost[-1] != '}':
        raise ValueError('Cost must start with { and end with }')
    result = ''
    for part in cost[1:-1].split('}{'):
        result += cost_part_to_mse(part)
    return result

def implicit_colors(cost, short=False):
    def cost_part_colors(part):
        basics = '[WUBRG]'
        if re.fullmatch(basics, part):
            # colored mana
            return {COLOR_ABBREVIATIONS[part]}
        if part == 'C':
            # colorless mana
            return set()
        if part == 'S':
            # snow mana
            return set()
        if part == 'X':
            # variable mana
            return set()
        if re.fullmatch('[0-9]+', part):
            # colorless mana
            return set()
        if re.fullmatch('{}/{}'.format(basics, basics), part):
            # colored/colored hybrid mana
            return {COLOR_ABBREVIATIONS[half] for half in part.split('/')}
        if re.fullmatch('{}/P'.format(basics), part):
            # Phyrexian mana
            return {COLOR_ABBREVIATIONS[part[0]]}
        if re.fullmatch('2/{}'.format(basics), part):
            # colorless/colored hybrid mana
            return {COLOR_ABBREVIATIONS[part[2]]}
        raise ValueError('Unknown mana cost part: {{{}}}'.format(part))

    if cost is None or cost == '':
        return []
    if cost[0] != '{' or cost[-1] != '}':
        raise ValueError('Cost must start with { and end with }')
    colors = set()
    for part in cost[1:-1].split('}{'):
        colors |= cost_part_colors(part)
    result = []
    for color in ('White', 'Blue', 'Black', 'Red', 'Green'):
        if color in colors:
            if short:
                result.append({
                    'White': 'W',
                    'Blue': 'U',
                    'Black': 'B',
                    'Red': 'R',
                    'Green': 'G'
                }[color])
            else:
                result.append(color)
    return result

def mtg_json(*, extras=False, verbose=False):
    if extras:
        if CACHE['db_x'] is None:
            if verbose:
                print('[....] downloading MTG JSON', end='', flush=True, file=sys.stderr)
            CACHE['db'] = CACHE['db_x'] = mtgjson.CardDb.from_url(mtgjson.ALL_SETS_X_ZIP_URL)
            if verbose:
                print('\r[ ok ]', file=sys.stderr)
        return CACHE['db_x']
    else:
        if CACHE['db'] is None:
            if verbose:
                print('[....] downloading MTG JSON', end='', flush=True, file=sys.stderr)
            CACHE['db'] = mtgjson.CardDb.from_url()
            if verbose:
                print('\r[ ok ]', file=sys.stderr)
        return CACHE['db']

if __name__ == '__main__':
    try:
        args = CommandLineArgs()
    except ValueError as e:
        sys.exit(f'[!!!!] {e.args[0]}')
    # read card names
    if sys.stdin.isatty():
        card_names = args.cards
    else:
        card_names = set(line.strip() for line in sys.stdin) | args.cards
    if len(card_names) == 0:
        sys.exit('[!!!!] missing card name')
    # download MTG JSON
    db = mtg_json(verbose=args.verbose)
    # normalize card names (DFC, split cards, etc)
    normalized_card_names = set()
    for card_name in card_names:
        match = re.fullmatch('(.+?) ?/+ ?.+', card_name)
        if match:
            card_name = match.group(1)
        try:
            card = db.cards_by_name[card_name]
        except KeyError:
            sys.exit(f'[!!!!] card not found: {card_name}')
        if 'names' in card._get_raw_data():
            normalized_card_names.add(card.names[0])
        else:
            normalized_card_names.add(card_name)
    # create set metadata
    set_file = MSEDataFile()
    set_file['mse version'] = '0.3.8'
    set_file['game'] = 'magic'
    set_file['stylesheet'] = 'm15'
    set_info = {
        'title': 'MTG JSON card import',
        'copyright': args.copyright,
        'description': '{} automatically imported from MTG JSON using json-to-mse.'.format('This card was' if len(normalized_card_names) == 1 else 'These cards were'),
        'set code': args.set_code,
        'set language': 'EN'
    }
    if not args.old_wedge_order:
        set_info['wedge mana costs'] = 'yes'
    if args.border_color is not None:
        set_info['border color'] = args.border_color
    set_file['set info'] = set_info
    set_file['styling'] = { # styling needs to be above cards
        'magic-m15': {
            'text box mana symbols': 'magic-mana-small.mse-symbol-font',
            'center text': 'short text only',
            'overlay': ''
        }
    }
    # add cards to set
    failed = 0
    for i, card_name in enumerate(sorted(normalized_card_names)):
        if args.verbose:
            progress = min(4, 5 * i // len(normalized_card_names))
            print('[{}{}] adding cards to set file: {} of {}'.format('=' * progress, '.' * (4 - progress), i, len(normalized_card_names)), end='\r', flush=True, file=sys.stderr)
        card = db.cards_by_name[card_name]
        try:
            set_file.add_card(card, db)
        except Exception as e:
            if args.verbose:
                raise RuntimeError(f'Failed to add card {card_name}') from e
            else:
                print(f'[ !! ] Failed to add card {card_name}        ', file=sys.stderr)
                failed += 1
    if failed > 0:
        print('[ ** ] Run again with --verbose for a detailed error message', file=sys.stderr)
    if args.verbose:
        print('[ ok ] adding cards to set file: {0} of {0}'.format(len(normalized_card_names)), file=sys.stderr)
    # generate stylesheet settings
    if hasattr(set_file, 'stylesheets'):
        for stylesheet in set_file.stylesheets:
            set_file['styling'][f'magic-{stylesheet}'] = {
                'text box mana symbols': 'magic-mana-small.mse-symbol-font',
                'center text': 'short text only',
                'overlay': ''
            }
    # zip and write set file
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'x') as f:
        f.writestr('set', str(set_file))
    args.output.write(buf.getvalue())
    args.output.flush()
