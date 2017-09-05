#!/usr/bin/env python3

import sys

import contextlib
import enum
import io
import more_itertools
import mtgjson
import pathlib
import re
import requests
import shlex
import subprocess
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
        self.border_color = None
        self.cards = set()
        self.copyright = 'NOT FOR SALE'
        self.decklists = set()
        self.find_cards = pathlib.Path('git/github.com/taw/magic-search-engine/master/search-engine/bin/find_cards')
        self._include_planes = None
        self._include_vanguards = None
        self.new_wedge_order = False
        self.output = sys.stdout.buffer
        self.planes_output = None
        self.queries = set()
        self.set_code = 'PROXY'
        self.vanguards_output = None
        self.verbose = False
        mode = None
        for arg in args:
            if mode == 'border':
                self.set_border_color(arg)
                mode = None
            elif mode == 'copyright':
                self.copyright = arg
                mode = None
            elif mode == 'find-cards':
                self.find_cards = pathlib.Path(arg)
                mode = None
            elif mode == 'input':
                self.set_input(arg)
                mode = None
            elif mode == 'output':
                self.output = open(arg, 'wb')
                mode = None
            elif mode == 'planes-output':
                self.planes_output = open(arg, 'wb')
                mode = None
            elif mode == 'set-code':
                self.set_code = arg
                mode = None
            elif mode == 'vanguards-output':
                self.vanguards_output = open(arg, 'wb')
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
                    elif arg == '--find-cards':
                        mode = 'find-cards'
                    elif arg.startswith('--find-cards='):
                        self.find_cards = pathlib.Path(arg[len('--find-cards='):])
                    elif arg == '--include-planes':
                        self.include_planes = True
                    elif arg == '--no-include-planes':
                        self.include_planes = False
                    elif arg == '--include-vanguards':
                        self.include_vanguards = True
                    elif arg == '--no-include-vanguards':
                        self.include_vanguards = False
                    elif arg == '--input':
                        mode = 'input'
                    elif arg.startswith('--input='):
                        self.set_input(arg[len('--input='):])
                    elif arg == '--new-wedge-order':
                        self.new_wedge_order = True
                    elif arg == '--output':
                        mode = 'output'
                    elif arg.startswith('--output='):
                        self.output = open(arg[len('--output='):], 'wb')
                    elif arg == '--planes-output':
                        mode = 'planes-output'
                    elif arg.startswith('--planes-output='):
                        self.planes_output = open(arg[len('--planes-output='):], 'wb')
                    elif arg == '--set-code':
                        mode = 'set-code'
                    elif arg.startswith('--set-code='):
                        self.set_code = arg[len('--set-code='):]
                    elif arg == '--vanguards-output':
                        mode = 'vanguards-output'
                    elif arg.startswith('--vanguards-output='):
                        self.vanguards_output = open(arg[len('--vanguards-output='):], 'wb')
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
                self.parse_input(arg)

    @property
    def include_planes(self):
        if self._include_planes is None:
            return self.planes_output is None
        else:
            return self._include_planes

    @include_planes.setter
    def include_planes(self, value):
        self._include_planes = value

    @property
    def include_vanguards(self):
        if self._include_vanguards is None:
            return self.vanguards_output is None
        else:
            return self._include_vanguards

    @include_vanguards.setter
    def include_vanguards(self, value):
        self._include_vanguards = value

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

    def parse_input(self, input_line):
        line = input_line.strip()
        if line == '':
            return
        if line.startswith('!'):
            cmd, *args = shlex.split(line[1:])
            if cmd == 'tappedout':
                self.decklists.add(f'http://tappedout.net/mtg-decks/{args[0]}/?fmt=txt')
            else:
                raise ValueError(f'Unrecognized input command: {cmd}')
            return
        if line.startswith('#'):
            return
        if line.startswith('='):
            self.queries.add(line[1:])
            return
        self.cards.add(line)

    def set_input(self, input_filename):
        with open(input_filename) as f:
            for line in f:
                self.parse_input(line)

class FrameFeatures(enum.Flag):
    NONE = 0
    AFTERMATH = enum.auto()
    CONSPIRACY = enum.auto()
    DEVOID = enum.auto()
    DFC = enum.auto()
    DRAFT_MATTERS = enum.auto()
    FLIP = enum.auto()
    FUSE = enum.auto()
    LEVELER = enum.auto()
    MIRACLE = enum.auto()
    NYX = enum.auto()
    PLANESWALKER = enum.auto()
    PLANESWALKER_BACK = enum.auto()
    SPLIT = enum.auto()
    TRUE_COLORLESS = enum.auto()
    TRUE_COLORLESS_BACK = enum.auto()
    VEHICLE = enum.auto()

    def alt_dfc(self):
        try:
            return {
                FrameFeatures.NONE: FrameFeatures.NONE,
                FrameFeatures.PLANESWALKER: FrameFeatures.PLANESWALKER_BACK,
                FrameFeatures.TRUE_COLORLESS: FrameFeatures.TRUE_COLORLESS_BACK
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

    def add(self, key, value):
        if isinstance(value, dict):
            value = MSEDataFile(value)
        elif value is True:
            value = 'true'
        elif value is False:
            value = 'false'
        self.items.append((key, value))

    def add_card(self, card_info, db, layout=None):
        card = self.__class__.from_card(card_info, db, layout=layout)
        self.add('card', card)
        with contextlib.suppress(KeyError):
            stylesheet = card['stylesheet']
            if not hasattr(self, 'stylesheets'):
                self.stylesheets = set()
            self.stylesheets.add(stylesheet)

    @classmethod
    def from_card(cls, card_info, db, layout=None, image=None, *, alt=False):
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
        if layout is None:
            if card_info.layout in ('normal', 'plane', 'phenomenon', 'vanguard'):
                pass # nothing specific to these layouts
            elif card_info.layout == 'double-faced':
                if not alt:
                    frame_features |= FrameFeatures.DFC
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, layout=layout, alt=2)
                    result |= alt_result
                    frame_features |= alt_frame_features.alt_dfc()
            elif card_info.layout == 'flip':
                if not alt:
                    frame_features |= FrameFeatures.FLIP
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, layout=layout, alt=2)
                    result |= alt_result
            elif card_info.layout == 'leveler':
                frame_features |= FrameFeatures.LEVELER
            elif card_info.layout == 'split':
                if not alt:
                    frame_features |= FrameFeatures.SPLIT
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, layout=layout, alt=2)
                    result |= alt_result
                    frame_features |= alt_frame_features
            else:
                raise NotImplementedError(f'Unsupported layout: {card_info.layout}') #TODO scheme, meld, aftermath
        elif layout == 'planechase':
            if card_info.layout in ('plane', 'phenomenon'):
                pass # nothing specific to these layouts
            else:
                raise NotImplementedError(f'Unsupported layout: {card_info.layout}')
        elif layout == 'vanguard':
            if card_info.layout == 'vanguard':
                pass # nothing specific to this layout
            else:
                raise NotImplementedError(f'Unsupported layout: {card_info.layout}')
        else:
            raise NotImplementedError(f'Unsupported MSE game: {layout}')
        # name
        result[alt_key('name')] = card_info.name
        # mana cost
        if 'manaCost' in raw_data:
            result[alt_key('casting cost')] = cost_to_mse(card_info.manaCost)
            #TODO check to add hybrid frame feature
        # image
        if image is not None:
            result[alt_key('image')] = image
        # color indicator
        if set(raw_data.get('colors', [])) != set(implicit_colors(raw_data.get('manaCost'))):
            if raw_data.get('colors', []) == []:
                frame_features |= FrameFeatures.DEVOID
                if image is not None:
                    result[alt_key('card color')] = ', '.join(c.lower() for c in implicit_colors(card_info.manaCost))
            else:
                result[alt_key('card color')] = result[alt_key('indicator')] = ', '.join(c.lower() for c in card_info.colors)
                #TODO make sure MSE renders two-color gold cards in the correct order
                #TODO make sure MSE renders 3+ color gold cards without the gradient
        elif set(raw_data.get('colors', [])):
            frame_features |= FrameFeatures.TRUE_COLORLESS
        # type line
        if layout == 'vanguard':
            result[alt_key('type')] = ' '.join((card_info.supertypes if 'supertypes' in raw_data else []) + card_info.types)
        elif 'supertypes' in raw_data:
            result[alt_key('supertype' if layout == 'planechase' else 'super type')] = f'<word-list-type>{" ".join(card_info.supertypes)} {" ".join(card_info.types)}</word-list-type>'
        else:
            result[alt_key('supertype' if layout == 'planechase' else 'super type')] = f'<word-list-type>{" ".join(card_info.types)}</word-list-type>'
        if 'subtypes' in raw_data:
            if 'Creature' in card_info.types:
                card_type = 'race'
            elif 'Instant' in card_info.types or 'Sorcery' in card_info.types:
                card_type = 'spell'
            else:
                card_type = card_info.types[0].lower()
            result[alt_key('subtype' if layout == 'planechase' else 'sub type')] = ' '.join(f'<word-list-{card_type}>{subtype}</word-list-race>' for subtype in card_info.subtypes)
        if 'Conspiracy' in card_info.types:
            frame_features |= FrameFeatures.CONSPIRACY
        if 'Planeswalker' in card_info.types:
            frame_features |= FrameFeatures.PLANESWALKER
        if 'Enchantment' in card_info.types and more_itertools.ilen(card_type for card_type in card_info.types if card_type != 'Tribal') >= 2:
            frame_features |= FrameFeatures.NYX
        if 'subtypes' in raw_data and 'Vehicle' in card_info.subtypes:
            frame_features |= FrameFeatures.VEHICLE
        # rarity
        result[alt_key('rarity')] = min(Rarity.from_str(printing.rarity) for printing in printings.values()).mse_str
        # text
        if 'text' in raw_data:
            striations = []
            text = ''
            for i, ability in enumerate(card_info.text.replace('‘', "'").split('\n')):
                ability = re.sub(' ?\\([^)]+\\)', '', ability)
                if ability == '':
                    continue
                elif ability == 'Fuse':
                    frame_features |= FrameFeatures.FUSE
                    continue
                elif card_info.layout == 'leveler':
                    match = re.fullmatch('LEVEL ([0-9]+)-([0-9]+)', ability)
                    if match:
                        if len(striations) > 0:
                            striations[-1]['text'] = text
                        else:
                            result[alt_key('rule text')] = text
                        text = ''
                        striations.append({
                            'from': int(match.group(1)),
                            'to': int(match.group(2))
                        })
                        continue
                    match = re.fullmatch('LEVEL ([0-9]+)\\+', ability)
                    if match:
                        if len(striations) > 0:
                            striations[-1]['text'] = text
                        else:
                            result[alt_key('rule text')] = text
                        text = ''
                        striations.append({
                            'from': int(match.group(1)),
                            'to': None
                        })
                        continue
                    if len(striations) > 0 and 'power' not in striations[-1]:
                        striations[-1]['power'], striations[-1]['toughness'] = ability.split('/')
                        continue
                match = re.fullmatch('((?:\\+|-|\u2212)(?:[0-9]+|X)|0): (.*)', ability)
                if 'Planeswalker' in card_info.type and match:
                    result[f'loyalty cost {4 * (alt or 1) + i - 3}'] = match.group(1).replace('\u2212', '-')
                    ability = match.group(2)
                if text != '':
                    if ability.startswith('•') or (layout == 'planechase' and not ability.startswith('Whenever you roll {CHAOS}')):
                        text += '<soft-line>\n</soft-line>'
                    else:
                        text += '\n'
                for j, word in enumerate(ability.split(' ')):
                    if j > 0:
                        text += ' '
                    if j == 0 and word == 'Miracle':
                        frame_features |= FrameFeatures.MIRACLE
                    if re.fullmatch('[Dd]raft(ed)?', word):
                        frame_features |= FrameFeatures.DRAFT_MATTERS
                    match = re.fullmatch('(["\']?)(\\{.+\\})([:.,]?)', word)
                    if match:
                        text += f'{match.group(1)}<sym>{cost_to_mse(match.group(2))}</sym>{match.group(3)}'
                    elif re.fullmatch('[0-9]+|X', word):
                        text += f'</sym>{word}<sym>'
                    else:
                        text += word
            if len(striations) > 0:
                striations[-1]['text'] = text
            else:
                result[alt_key('rule text')] = text
            for i, striation in enumerate(striations):
                if striation['to'] is None:
                    result[f'level {i + 1}'] = f'{striation["from"]}+'
                else:
                    result[f'level {i + 1}'] = f'{striation["from"]}-{striation["to"]}'
                result[f'rule text {i + 2}'] = striation['text']
                result[f'power {i + 2}'] = striation['power']
                result[f'toughness {i + 2}'] = striation['toughness']
        # mana symbol
        if alt_key('rule text') not in result or result[alt_key('rule text')] == '':
            if 'subtypes' in raw_data and more_itertools.quantify(subtype in BASIC_LAND_TYPES for subtype in card_info.subtypes) == 1:
                subtype = more_itertools.one(subtype for subtype in card_info.subtypes if subtype in BASIC_LAND_TYPES)
                result[alt_key('watermark')] = 'mana symbol {}'.format(COLOR_ABBREVIATIONS[BASIC_LAND_TYPES[subtype]].lower())
        # P/T
        if 'power' in raw_data:
            result[alt_key('power')] = card_info.power
        if 'toughness' in raw_data:
            result[alt_key('toughness')] = card_info.toughness
        # loyalty
        if 'loyalty' in raw_data:
            result[alt_key('loyalty')] = card_info.loyalty
        # hand/life modifier
        if 'hand' in raw_data:
            result[alt_key('handmod' if layout == 'vanguard' else 'power')] = f'{card_info.hand:+}'
        if 'life' in raw_data:
            result[alt_key('lifemod' if layout == 'vanguard' else 'toughness')] = f'{card_info.life:+}'
        # stylesheet
        if alt:
            return result, frame_features
        elif layout == 'planechase':
            if 'Phenomenon' in card_info.types:
                result['stylesheet'] = 'phenomenon'
            return result
        elif layout == 'vanguard':
            return result
        else:
            if FrameFeatures.SPLIT in frame_features:
                if FrameFeatures.FUSE in frame_features:
                    result['stylesheet'] = 'm15-split-fuse'
                    result['rule text 3'] = 'Fuse' #TODO reminder text based on options
                elif FrameFeatures.AFTERMATH in frame_features:
                    raise NotImplementedError('Aftermath not implemented') #TODO
                else:
                    result['stylesheet'] = 'm15-split'
            elif FrameFeatures.FLIP in frame_features:
                result['stylesheet'] = 'm15-flip'
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
                result['stylesheet'] = 'm15-planeswalker-2abil'
            elif FrameFeatures.LEVELER in frame_features:
                result['stylesheet'] = 'm15-leveler'
            elif FrameFeatures.CONSPIRACY in frame_features:
                result['stylesheet'] = 'm15-ttk-conspiracy'
                result['watermark'] = 'other magic symbols conspiracy stamp'
            elif FrameFeatures.DRAFT_MATTERS in frame_features:
                result['stylesheet'] = 'm15-ttk-frames'
                result['watermark'] = 'other magic symbols conspiracy stamp'
            elif FrameFeatures.MIRACLE in frame_features:
                result['stylesheet'] = 'm15-miracle'
            elif FrameFeatures.DEVOID in frame_features:
                if image is None: #TODO or if image isn't full art
                    # don't use devoid frame because it assumes full art
                    pass # m15-extra seems to be broken
                    #result['stylesheet'] = 'm15-extra'
                    #result['has styling'] = True
                    #result['styling data'] = {
                    #    'outer color': 'colorless',
                    #    'trim color': 'colorless',
                    #    'inner color': 'default'
                    #}
                else:
                    result['stylesheet'] = 'm15-devoid'
            elif FrameFeatures.VEHICLE in frame_features:
                result['stylesheet'] = 'vehicles'
            elif FrameFeatures.NYX in frame_features:
                result['stylesheet'] = 'm15-nyx'
            return result

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
        if part in ('C', 'E', 'Q', 'S', 'T', 'X'):
            # colorless mana, energy counter, untap symbol, snow mana, tap symbol, variable mana
            return part
        if part == 'CHAOS':
            # chaos symbol (planar die)
            return 'A'
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
    card_names = args.cards
    if not sys.stdin.isatty():
        card_names |= set(line.strip() for line in sys.stdin)
    for i, decklist_url in enumerate(args.decklists):
        if args.verbose:
            progress = min(4, 5 * i // len(args.decklists))
            print('[{}{}] downloading decklists: {} of {}'.format('=' * progress, '.' * (4 - progress), i, len(args.decklists)), end='\r', flush=True, file=sys.stderr)
        response = requests.get(decklist_url)
        response.encoding = 'utf-8'
        card_names |= {
            line.split(' ', 1)[1].replace('’', "'")
            for line in response.text.splitlines()
            if line not in ('', 'Sideboard:')
        }
    if args.verbose and len(args.decklists) > 0:
        print('[ ok ] downloading decklists: {0} of {0}'.format(len(args.decklists)), file=sys.stderr)
    for query in args.queries:
        if args.verbose:
            print(f'[....] finding cards: {query}', end='\r', flush=True, file=sys.stderr)
        card_names |= set(subprocess.run(['ruby', '--encoding=UTF-8:UTF-8', str(args.find_cards), query], stdout=subprocess.PIPE).stdout.decode('utf-8').splitlines())
        if args.verbose:
            print('[ ok ]', file=sys.stderr)
    if len(card_names) == 0:
        sys.exit('[!!!!] missing card name')
    # download MTG JSON
    db = mtg_json(verbose=args.verbose)
    # normalize card names (DFC, split cards, etc)
    normalized_card_names = set()
    for card_name in sorted(card_names):
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
        'set language': 'EN',
        'mark errors': 'no',
        'automatic reminder text': ''
    }
    if args.new_wedge_order:
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
    planes_set_file = MSEDataFile()
    planes_set_file['mse version'] = '0.3.8'
    planes_set_file['game'] = 'planechase'
    planes_set_file['stylesheet'] = 'standard'
    planes_set_file['set info'] = {
        'title': 'MTG JSON card import: planes and phenomena',
        'copyright': args.copyright,
        'description': '{} automatically imported from MTG JSON using json-to-mse.'.format('This card was' if len(normalized_card_names) == 1 else 'These cards were'),
        'set code': args.set_code,
        'set language': 'EN',
        'mark errors': 'no',
        'automatic reminder text': '',
        'automatic card numbers': 'no'
    }
    planes_set_file['styling'] = { # styling needs to be above cards
        'planechase-standard': {
            'text box mana symbols': 'magic-mana-small.mse-symbol-font',
            'tap symbol': 'modern'
        },
        'planechase-phenomenon': {
            'text box mana symbols': 'magic-mana-small.mse-symbol-font',
            'tap symbol': 'modern'
        }
    }
    vanguards_set_file = MSEDataFile()
    vanguards_set_file['mse version'] = '0.3.8'
    vanguards_set_file['game'] = 'vanguard'
    vanguards_set_file['stylesheet'] = 'standard'
    vanguards_set_info = {
        'title': 'MTG JSON card import: vanguards',
        'copyright': args.copyright,
        'description': '{} automatically imported from MTG JSON using json-to-mse.'.format('This card was' if len(normalized_card_names) == 1 else 'These cards were'),
        'set code': args.set_code,
        'set language': 'EN',
        'mark errors': 'no',
        'automatic reminder text': '',
        'automatic card numbers': 'no'
    }
    if args.border_color is not None:
        vanguards_set_info['border color'] = args.border_color
    vanguards_set_file['set info'] = vanguards_set_info
    vanguards_set_file['styling'] = { # styling needs to be above cards
        'vanguard-standard': {
            'text box mana symbols': 'magic-mana-small.mse-symbol-font',
            'tap symbol': 'modern',
            'flavor text': 'no'
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
            if 'Plane' in card.types or 'Phenomenon' in card.types:
                if args.include_planes:
                    set_file.add_card(card, db)
                planes_set_file.add_card(card, db, layout='planechase')
            elif 'Vanguard' in card.types:
                if args.include_vanguards:
                    set_file.add_card(card, db)
                vanguards_set_file.add_card(card, db, layout='vanguard')
            else:
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
            styling = {
                'text box mana symbols': 'magic-mana-small.mse-symbol-font',
                'overlay': ''
            }
            if stylesheet == 'm15-leveler':
                pass # keep leveler cards left-aligned
            elif stylesheet in ('m15-split', 'm15-split-fuse'):
                styling['center text 1'] = styling['center text 2'] = 'always'
            else:
                styling['center text'] = 'short text only'
            set_file['styling'][f'magic-{stylesheet}'] = styling
    # generate footers
    set_file['version control'] = planes_set_file['version control'] = {'type': 'none'}
    set_file['apprentice code'] = planes_set_file['apprentice code'] = ''
    # zip and write set files
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'x') as f:
        f.writestr('set', str(set_file))
    args.output.write(buf.getvalue())
    args.output.flush()
    if args.planes_output is not None:
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'x') as f:
            f.writestr('set', str(planes_set_file))
        args.planes_output.write(buf.getvalue())
        args.planes_output.flush()
    if args.vanguards_output is not None:
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'x') as f:
            f.writestr('set', str(vanguards_set_file))
        args.vanguards_output.write(buf.getvalue())
        args.vanguards_output.flush()
