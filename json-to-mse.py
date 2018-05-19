#!/usr/bin/env python3

import sys

import PIL.Image
import contextlib
import enum
import io
import more_itertools
import mtgjson
import pathlib
import regex
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

BLACK_BORDERED_UNCARDS = [
    '1996 World Champion',
    'Fraternal Exaltation',
    'Proposal',
    'Robot Chicken',
    'Shichifukujin Dragon',
    'Splendid Genesis'
]

COLOR_ABBREVIATIONS = {
    'W': 'White',
    'U': 'Blue',
    'B': 'Black',
    'R': 'Red',
    'G': 'Green'
}

class UncardError(ValueError):
    pass

class CommandLineArgs:
    def __init__(self, args=sys.argv[1:]):
        self.all_command = False
        self.allow_uncards = False
        self.auto_card_numbers = False
        self.border_color = None
        self.cards = set()
        self.copyright = 'NOT FOR SALE'
        self.decklists = set()
        self.find_cards = pathlib.Path('git/github.com/taw/magic-search-engine/master/search-engine/bin/find_cards')
        self.images = None
        self._include_planes = None
        self._include_schemes = None
        self._include_vanguards = None
        self.new_wedge_order = False
        self.output = sys.stdout.buffer
        self.planes_output = None
        self.queries = set()
        self.schemes_output = None
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
            elif mode == 'images':
                self.images = pathlib.Path(arg)
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
            elif mode == 'schemes-output':
                self.schemes_output = open(arg, 'wb')
                mode = None
            elif mode == 'set-code':
                self.set_code = arg
                mode = None
            elif mode == 'vanguards-output':
                self.vanguards_output = open(arg, 'wb')
                mode = None
            elif arg.startswith('-'):
                if arg.startswith('--'):
                    if arg == '--allow-uncards':
                        self.allow_uncards = True
                    elif arg == '--auto-card-numbers':
                        self.auto_card_numbers = True
                    elif arg == '--border':
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
                    elif arg == '--images':
                        mode = 'images'
                    elif arg.startswith('--images='):
                        self.images = pathlib.Path(arg[len('--images='):])
                    elif arg == '--include-planes':
                        self.include_planes = True
                    elif arg == '--no-include-planes':
                        self.include_planes = False
                    elif arg == '--include-schemes':
                        self.include_schemes = True
                    elif arg == '--no-include-schemes':
                        self.include_schemes = False
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
                    elif arg == '--schemes-output':
                        mode = 'schemes-output'
                    elif arg.startswith('--schemes-output='):
                        self.schemes_output = open(arg[len('--schemes-output='):], 'wb')
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
    def include_schemes(self):
        if self._include_schemes is None:
            return self.schemes_output is None
        else:
            return self._include_schemes

    @include_schemes.setter
    def include_schemes(self, value):
        self._include_schemes = value

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
            if cmd == 'all':
                self.all_command = True
            elif cmd == 'tappedout':
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
        input_path = pathlib.Path(input_filename)
        if input_path.is_dir():
            if self.images is None:
                self.images = input_path
            self.cards |= {
                image_path.stem
                for image_path in input_path.iterdir()
                if image_path.suffix == '.png'
            }
        else:
            with input_path.open() as f:
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
    FULL_ART_LAND = enum.auto()
    FUSE = enum.auto()
    LAND = enum.auto()
    LAND_BACK = enum.auto()
    LEVELER = enum.auto()
    MELD = enum.auto()
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
                FrameFeatures.LAND: FrameFeatures.LAND_BACK,
                FrameFeatures.NONE: FrameFeatures.NONE,
                FrameFeatures.PLANESWALKER: FrameFeatures.PLANESWALKER_BACK,
                FrameFeatures.TRUE_COLORLESS: FrameFeatures.TRUE_COLORLESS_BACK
            }[self]
        except KeyError as e:
            raise NotImplementedError('Frame features {} not implemented for DFC back faces'.format(self.name)) from e

class MSEDataFile:
    def __init__(self, data={}):
        self.images = []
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

    def add_card(self, card_info, db, **kwargs):
        card = self.__class__.from_card(card_info, db, default_stylesheet=self['stylesheet'], images_to_add=self.images, **kwargs)
        self.add('card', card)
        with contextlib.suppress(KeyError):
            stylesheet = card['stylesheet']
            if not hasattr(self, 'stylesheets'):
                self.stylesheets = set()
            self.stylesheets.add(stylesheet)

    @classmethod
    def from_card(cls, card_info, db, default_stylesheet='m15-extra', layout=None, images=None, images_to_add=None, new_wedge_order=False, allow_uncards=False, *, alt=False):
        def alt_key(key_name):
            if alt:
                return f'{key_name} {alt}'
            else:
                return key_name

        # check for legality
        if card_info.layout == 'token':
            raise ValueError('Token cards are not supported')
        if not allow_uncards:
            if getattr(card_info, 'border', card_info.set.border) == 'silver':
                raise UncardError('Un-cards are not supported')
            if card_info.name in BLACK_BORDERED_UNCARDS:
                raise UncardError('This card is blacklisted and will not be supported')
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
            if card_info.layout in ('normal', 'plane', 'phenomenon', 'scheme', 'vanguard'):
                pass # nothing specific to these layouts
            elif card_info.layout == 'aftermath':
                if not alt:
                    frame_features |= FrameFeatures.SPLIT
                    frame_features |= FrameFeatures.AFTERMATH
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, default_stylesheet=default_stylesheet, layout=layout, images=images, images_to_add=images_to_add, new_wedge_order=new_wedge_order, allow_uncards=allow_uncards, alt=2)
                    result |= alt_result
                    frame_features |= alt_frame_features
            elif card_info.layout == 'double-faced':
                if not alt:
                    frame_features |= FrameFeatures.DFC
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, default_stylesheet=default_stylesheet, layout=layout, images=images, images_to_add=images_to_add, new_wedge_order=new_wedge_order, allow_uncards=allow_uncards, alt=2)
                    result |= alt_result
                    frame_features |= alt_frame_features.alt_dfc()
            elif card_info.layout == 'flip':
                if not alt:
                    frame_features |= FrameFeatures.FLIP
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, default_stylesheet=default_stylesheet, layout=layout, images=images, images_to_add=images_to_add, new_wedge_order=new_wedge_order, allow_uncards=allow_uncards, alt=2)
                    result |= alt_result
            elif card_info.layout == 'leveler':
                frame_features |= FrameFeatures.LEVELER
            elif card_info.layout == 'meld':
                if not alt:
                    frame_features |= FrameFeatures.DFC
                    frame_features |= FrameFeatures.MELD
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[2]], db, default_stylesheet=default_stylesheet, layout=layout, images=images, images_to_add=images_to_add, new_wedge_order=new_wedge_order, allow_uncards=allow_uncards, alt=2)
                    result |= alt_result
                    frame_features |= alt_frame_features.alt_dfc()
            elif card_info.layout == 'split':
                if not alt:
                    frame_features |= FrameFeatures.SPLIT
                    alt_result, alt_frame_features = cls.from_card(db.cards_by_name[card_info.names[1]], db, default_stylesheet=default_stylesheet, layout=layout, images=images, images_to_add=images_to_add, new_wedge_order=new_wedge_order, allow_uncards=allow_uncards, alt=2)
                    result |= alt_result
                    frame_features |= alt_frame_features
            else:
                raise NotImplementedError(f'Unsupported layout: {card_info.layout}')
        elif layout == 'archenemy':
            if card_info.layout == 'scheme':
                pass # nothing specific to this layout
            else:
                raise NotImplementedError(f'Unsupported layout: {card_info.layout}')
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
        if hasattr(card_info, 'manaCost') and not (alt and card_info.layout == 'flip'):
            result[alt_key('casting cost')] = cost_to_mse(card_info.manaCost, normalize=new_wedge_order)
        # image
        if images is not None and images_to_add is not None and (images / f'{card_info.name}.png').exists():
            image = images / f'{card_info.name}.png'
            result[alt_key('image')] = f'image{len(images_to_add) + 1}'
            images_to_add.append(image)
            with PIL.Image.open(image) as img:
                image_is_vertical = img.size[1] > img.size[0]
        else:
            image = None
            image_is_vertical = False
        # frame color & color indicator
        frame_color = []
        if getattr(card_info, 'colors', []) == []:
            if 'Artifact' not in card_info.types:
                frame_color.append('colorless')
        elif len(card_info.colors) > 2:
            frame_color.append('multicolor')
        else:
            frame_color += [c.lower() for c in card_info.colors]
        if 'Artifact' in card_info.types:
            frame_color.append('artifact')
        if 'Land' in card_info.types:
            frame_color.append('land')
        if 'Land' in card_info.types:
            if getattr(card_info, 'colors', []) == []:
                land_colors = [c.lower() for c in could_produce(card_info) if c != 'Colorless']
                if len(land_colors) > 2:
                    frame_color = 'multicolor, land'
                elif len(land_colors) > 0:
                    frame_color = ', '.join(land_colors) + ', land'
                else:
                    frame_color = 'land'
                result[alt_key('card color')] = frame_color
                result[alt_key('indicator')] = 'colorless'
            else:
                result[alt_key('card color')] = ', '.join(frame_color)
                result[alt_key('indicator')] = ', '.join(c.lower() for c in card_info.colors)
        elif set(getattr(card_info, 'colors', [])) != set(implicit_colors(getattr(card_info, 'manaCost', None))):
            if getattr(card_info, 'colors', []) == []:
                if image_is_vertical:
                    frame_features |= FrameFeatures.DEVOID
                    result[alt_key('card color')] = ', '.join(c.lower() for c in implicit_colors(getattr(card_info, 'manaCost', None)))
                else:
                    result[alt_key('card color')] = ', '.join(frame_color)
            else:
                result[alt_key('card color')] = ', '.join(frame_color)
                result[alt_key('indicator')] = ', '.join(c.lower() for c in card_info.colors)
        elif getattr(card_info, 'colors', []) == []:
            if image_is_vertical and not any(card_type in card_info.types for card_type in ['Artifact', 'Land', 'Phenomenon', 'Plane', 'Scheme', 'Vanguard']):
                frame_features |= FrameFeatures.TRUE_COLORLESS
        # type line
        if layout == 'archenemy':
            type_line = ' '.join(getattr(card_info, 'supertypes', []) + card_info.types)
            if hasattr(card_info, 'subtypes'):
                # archenemy templates don't support subtypes, so include them with the card types
                type_line += ' — {" ".join(card_info.subtypes)}'
            result[alt_key('type')] = f'<word-list-type>{type_line}</word-list-type>'
        elif layout == 'vanguard':
            result[alt_key('type')] = ' '.join(getattr(card_info, 'supertypes', []) + card_info.types)
        elif hasattr(card_info, 'supertypes'):
            result[alt_key('supertype' if layout == 'planechase' else 'super type')] = f'<word-list-type>{" ".join(card_info.supertypes)} {" ".join(card_info.types)}</word-list-type>'
        else:
            result[alt_key('supertype' if layout == 'planechase' else 'super type')] = f'<word-list-type>{" ".join(card_info.types)}</word-list-type>'
        if layout != 'archenemy' and hasattr(card_info, 'subtypes'):
            if 'Creature' in card_info.types:
                card_type = 'race'
            elif 'Instant' in card_info.types or 'Sorcery' in card_info.types:
                card_type = 'spell'
            else:
                card_type = card_info.types[0].lower()
            result[alt_key('subtype' if layout == 'planechase' else 'sub type')] = ' '.join(f'<word-list-{card_type}>{subtype}</word-list-race>' for subtype in card_info.subtypes)
        if 'Conspiracy' in card_info.types:
            frame_features |= FrameFeatures.CONSPIRACY
        if 'Land' in card_info.types:
            frame_features |= FrameFeatures.LAND
        if 'Planeswalker' in card_info.types:
            frame_features |= FrameFeatures.PLANESWALKER
        if 'Enchantment' in card_info.types and more_itertools.quantify(card_type != 'Tribal' for card_type in card_info.types) >= 2:
            frame_features |= FrameFeatures.NYX
        if 'Vehicle' in getattr(card_info, 'subtypes', []):
            frame_features |= FrameFeatures.VEHICLE
        # rarity
        result[alt_key('rarity')] = min(Rarity.from_str(printing.rarity) for printing in printings.values()).mse_str
        # text
        if hasattr(card_info, 'text'):
            striations = []
            text = ''
            for i, ability in enumerate(card_info.text.replace('‘', "'").split('\n')):
                ability = regex.sub(' ?\\([^)]+\\)', '', ability)
                if ability == '':
                    continue
                elif ability == 'Fuse':
                    frame_features |= FrameFeatures.FUSE
                    continue
                elif card_info.layout == 'leveler':
                    match = regex.fullmatch('LEVEL ([0-9]+)-([0-9]+)', ability)
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
                    match = regex.fullmatch('LEVEL ([0-9]+)\\+', ability)
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
                match = regex.fullmatch('((?:\\+|-|\u2212)(?:[0-9]+|X)|0): (.*)', ability)
                if 'Planeswalker' in card_info.types and match:
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
                    for k, word_part in enumerate(word.split('\u2014')): # em dash
                        if k > 0:
                            text += '\u2014'
                        if j == 0 and k == 0 and word_part == 'Miracle':
                            frame_features |= FrameFeatures.MIRACLE
                        if regex.fullmatch('[Dd]raft(ed)?', word_part):
                            frame_features |= FrameFeatures.DRAFT_MATTERS
                        match = regex.fullmatch('(["\']?)(\\{.+\\})([:.,]?["\']*)', word_part)
                        if match:
                            text += f'{match.group(1)}<sym>{cost_to_mse(match.group(2), normalize=new_wedge_order)}</sym>{match.group(3)}'
                        elif regex.fullmatch('[0-9]+|X', word_part):
                            text += f'</sym>{word_part}<sym>'
                        else:
                            text += word_part
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
        # layouts and mana symbol watermarks for vanilla cards
        if alt_key('rule text') not in result or result[alt_key('rule text')] == '':
            if more_itertools.quantify(subtype in BASIC_LAND_TYPES for subtype in getattr(card_info, 'subtypes', [])) == 1:
                subtype = more_itertools.one(subtype for subtype in card_info.subtypes if subtype in BASIC_LAND_TYPES)
                result[alt_key('watermark')] = 'mana symbol {}'.format(COLOR_ABBREVIATIONS[BASIC_LAND_TYPES[subtype]].lower())
            elif more_itertools.quantify(subtype in BASIC_LAND_TYPES for subtype in getattr(card_info, 'subtypes', [])) == 2:
                color1, color2 = (BASIC_LAND_TYPES[subtype] for subtype in card_info.subtypes if subtype in BASIC_LAND_TYPES)
                result[alt_key('watermark')] = 'colored xander hybrid mana {}/{}'.format(color1, color2)
            if 'Land' in card_info.types and image_is_vertical:
                frame_features |= FrameFeatures.FULL_ART_LAND
        # P/T
        if hasattr(card_info, 'power'):
            result[alt_key('power')] = card_info.power
        if hasattr(card_info, 'toughness'):
            result[alt_key('toughness')] = card_info.toughness
        # loyalty
        if hasattr(card_info, 'loyalty'):
            result[alt_key('loyalty')] = card_info.loyalty
        # hand/life modifier
        if hasattr(card_info, 'hand'):
            result[alt_key('handmod' if layout == 'vanguard' else 'power')] = f'{card_info.hand:+}'
        if hasattr(card_info, 'life'):
            result[alt_key('lifemod' if layout == 'vanguard' else 'toughness')] = f'{card_info.life:+}'
        #TODO artist credit
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
                    result['stylesheet'] = 'm15-aftermath'
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
                    elif FrameFeatures.TRUE_COLORLESS_BACK in frame_features:
                        result['stylesheet'] = 'm15-colored-to-clear'
                    elif FrameFeatures.LAND_BACK in frame_features:
                        result['stylesheet'] = 'm15-doublefaced-ixalands'
                    else:
                        result['stylesheet'] = 'm15-doublefaced'
                # face symbols depending on whether it's a meld card TODO add options to customize: all the same, or according to CR, or according to template (skipping this code)
                if 'extra data' not in result:
                    result['extra data'] = {}
                if result['stylesheet'] not in result['extra data']:
                    result['extra data'][result['stylesheet']] = {}
                if FrameFeatures.MELD in frame_features:
                    result['extra data'][result['stylesheet']]['corner'] = 'moon'
                    result['extra data'][result['stylesheet']]['corner 2'] = 'eldrazi'
                else:
                    result['extra data'][result['stylesheet']]['corner'] = 'day'
                    result['extra data'][result['stylesheet']]['corner 2'] = 'night'
            elif FrameFeatures.PLANESWALKER in frame_features:
                if FrameFeatures.TRUE_COLORLESS in frame_features:
                    result['stylesheet'] = 'm15-planeswalker-clear'
                else:
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
                result['stylesheet'] = 'm15-devoid'
            elif FrameFeatures.VEHICLE in frame_features:
                result['stylesheet'] = 'vehicles'
            elif FrameFeatures.NYX in frame_features:
                result['stylesheet'] = 'm15-nyx'
            elif FrameFeatures.TRUE_COLORLESS in frame_features:
                result['stylesheet'] = 'm15-clear'
            elif FrameFeatures.FULL_ART_LAND in frame_features:
                result['stylesheet'] = 'm15-textless-land'
            # make sure the frame color around the stamp matches color of the rest of the frame
            if alt_key('card color') in result:
                if 'extra data' not in result:
                    result['extra data'] = {}
                if result.get('stylesheet', default_stylesheet) not in result['extra data']:
                    result['extra data'][result.get('stylesheet', default_stylesheet)] = {}
                result['extra data'][result.get('stylesheet', default_stylesheet)]['stamp'] = result[alt_key('card color')]
            return result

    def get(self, key, default=None):
        for iter_key, value in self.items:
            if iter_key == key:
                return value
        else:
            return default

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

def cost_to_mse(cost, *, normalize=False):
    def canonical_order(result_list, symbols):
        counts = [0, 0, 0, 0, 0]
        for i in reversed(range(len(result_list))):
            for color, symbol in enumerate(symbols):
                if result_list[i] == symbol:
                    counts[color] += 1
                    del result_list[i]
                    break
        colors_present = tuple(count > 0 for count in counts)
        order = {
            #colorless
            (False, False, False, False, False): [],
            # single colors
            (True, False, False, False, False): [0],
            (False, True, False, False, False): [1],
            (False, False, True, False, False): [2],
            (False, False, False, True, False): [3],
            (False, False, False, False, True): [4],
            # allied pairs
            (True, True, False, False, False): [0, 1],
            (False, True, True, False, False): [1, 2],
            (False, False, True, True, False): [2, 3],
            (False, False, False, True, True): [3, 4],
            (True, False, False, False, True): [4, 0],
            # enemy pairs
            (True, False, True, False, False): [0, 2],
            (False, True, False, True, False): [1, 3],
            (False, False, True, False, True): [2, 4],
            (True, False, False, True, False): [3, 0],
            (False, True, False, False, True): [4, 1],
            # shards
            (True, True, False, False, True): [4, 0, 1],
            (True, True, True, False, False): [0, 1, 2],
            (False, True, True, True, False): [1, 2, 3],
            (False, False, True, True, True): [2, 3, 4],
            (True, False, False, True, True): [3, 4, 0],
            # wedges
            (True, False, True, False, True): [0, 2, 4],
            (True, True, False, True, False): [1, 3, 0],
            (False, True, True, False, True): [2, 4, 1],
            (True, False, True, True, False): [3, 0, 2],
            (False, True, False, True, True): [4, 1, 3],
            # nephilim
            (True, True, True, True, False): [0, 1, 2, 3],
            (False, True, True, True, True): [1, 2, 3, 4],
            (True, False, True, True, True): [2, 3, 4, 0],
            (True, True, False, True, True): [3, 4, 0, 1],
            (True, True, True, False, True): [4, 0, 1, 2],
            # rainbow
            (True, True, True, True, True): [0, 1, 2, 3, 4]
        }[colors_present]
        return ''.join(cost_part_to_mse(symbols[color]) * counts[color] for color in order)

    def cost_part_to_mse(part):
        basics = '[WUBRG]'
        if regex.fullmatch(basics, part):
            # colored mana
            return part
        if part in ('C', 'E', 'Q', 'S', 'T', 'X'):
            # colorless mana, energy counter, untap symbol, snow mana, tap symbol, variable mana
            return part
        if part == 'P':
            # any Phyrexian mana symbol
            return 'phi'
        if part == 'CHAOS':
            # chaos symbol (planar die)
            return 'chaos'
        if regex.fullmatch('[0-9]+', part):
            # colorless mana
            return part
        if regex.fullmatch('{}/{}'.format(basics, basics), part):
            # colored/colored hybrid mana
            return part
        match = regex.fullmatch('({})/P'.format(basics), part)
        if match:
            # Phyrexian mana
            return f'H/{match.group(1)}'
        if regex.fullmatch('2/{}'.format(basics), part):
            # colorless/colored hybrid mana
            return part
        raise ValueError('Unknown mana cost part: {{{}}}'.format(part))

    if cost is None or cost == '':
        return ''
    if cost[0] != '{' or cost[-1] != '}':
        raise ValueError('Cost must start with { and end with }')
    result_list = []
    for part in cost[1:-1].split('}{'):
        result_list.append(part)
    result = ''
    if normalize:
        # generic and colorless costs
        for i in reversed(range(len(result_list))):
            if result_list[i] == 'X':
                result += cost_part_to_mse('X')
                del result_list[i]
        total = 0
        for i in reversed(range(len(result_list))):
            try:
                generic = int(result_list[i])
            except ValueError:
                pass
            else:
                if generic == 0:
                    result += cost_part_to_mse('0')
                total += generic
                del result_list[i]
        if total > 0:
            result += cost_part_to_mse(str(total))
        for i in reversed(range(len(result_list))):
            if result_list[i] == 'S':
                result += cost_part_to_mse('S')
                del result_list[i]
        for i in reversed(range(len(result_list))):
            if result_list[i] == 'C':
                result += cost_part_to_mse('C')
                del result_list[i]
        # twobrid
        result += canonical_order(result_list, ['2/W', '2/U', '2/B', '2/R', '2/G'])
        # hybrid
        for symbol in ['W/U', 'U/B', 'B/R', 'R/G', 'G/W', 'W/B', 'U/R', 'B/G', 'R/W', 'G/U']:
            for i in reversed(range(len(result_list))):
                if result_list[i] == symbol:
                    result += cost_part_to_mse(symbol)
                    del result_list[i]
        # Phyrexian
        result += canonical_order(result_list, ['W/P', 'U/P', 'B/P', 'R/P', 'G/P'])
        # colored
        result += canonical_order(result_list, ['W', 'U', 'B', 'R', 'G'])
        # other
    return result + ''.join(cost_part_to_mse(part) for part in result_list)

def could_produce(card_info):
    """Returns the types of mana that could be produced by this card, assuming an empty game state."""
    # hardcoded edge cases
    if card_info.name == 'Gemstone Caverns':
        return {'Colorless'}
    #TODO make this more accurate
    result = set()
    for basic_land_type, mana_color in BASIC_LAND_TYPES.items():
        if basic_land_type in getattr(card_info, 'subtypes', []):
            result.add(COLOR_ABBREVIATIONS[mana_color])
    match = regex.search('(.*?add(,?( or)? (\{(?P<types>[CWUBRG])\})+)+)+', getattr(card_info, 'text', ''), regex.IGNORECASE | regex.DOTALL)
    if match:
        for mana_type in match.captures('types'):
            if mana_type == 'C':
                result.add('Colorless')
            else:
                result.add(COLOR_ABBREVIATIONS[mana_type])
    if regex.search('add (one|three) mana of any( one)? color', getattr(card_info, 'text', ''), regex.IGNORECASE):
        result |= {'White', 'Blue', 'Black', 'Red', 'Green'}
    if regex.search('add one mana of that color', getattr(card_info, 'text', ''), regex.IGNORECASE):
        if card_info.name == 'Rhystic Cave':
            result |= {'White', 'Blue', 'Black', 'Red', 'Green'}
        elif card_info.name == 'Meteor Crater':
            pass # Meteor Crater does not produce mana on an empty board
        else:
            raise NotImplementedError('could_produce for {} not implemented'.format(card_info.name))
    return result

def implicit_colors(cost, short=False):
    def cost_part_colors(part):
        basics = '[WUBRG]'
        if regex.fullmatch(basics, part):
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
        if regex.fullmatch('[0-9]+', part):
            # colorless mana
            return set()
        if regex.fullmatch('{}/{}'.format(basics, basics), part):
            # colored/colored hybrid mana
            return {COLOR_ABBREVIATIONS[half] for half in part.split('/')}
        if regex.fullmatch('{}/P'.format(basics), part):
            # Phyrexian mana
            return {COLOR_ABBREVIATIONS[part[0]]}
        if regex.fullmatch('2/{}'.format(basics), part):
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

def main():
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
        response.raise_for_status()
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
        card_names |= set(subprocess.run(['ruby', '--encoding=UTF-8:UTF-8', str(args.find_cards), query], stdout=subprocess.PIPE, check=True).stdout.decode('utf-8').splitlines())
        if args.verbose:
            print('[ ok ]', file=sys.stderr)
    if len(card_names) == 0 and not args.all_command:
        sys.exit('[!!!!] missing card name')
    # download MTG JSON
    db = mtg_json(verbose=args.verbose)
    if args.all_command:
        card_names |= {
            card_name
            for card_name, card_info in db.cards_by_name.items()
            if card_info.layout != 'token'
            and getattr(card_info, 'border', card_info.set.border) != 'silver'
            and card_name not in BLACK_BORDERED_UNCARDS
        }
    # normalize card names (DFC, split cards, etc)
    normalized_card_names = set()
    for card_name in sorted(card_names):
        match = regex.fullmatch('(.+?) ?/+ ?.+', card_name)
        if match:
            card_name = match.group(1)
        try:
            card = db.cards_by_name[card_name]
        except KeyError:
            sys.exit(f'[!!!!] card not found: {card_name}')
        if hasattr(card, 'names'):
            normalized_card_names.add(card.names[0])
            if card.layout == 'meld':
                normalized_card_names.add(card.names[1]) # generate both front faces separately. TODO option to generate a 3-in-1 tempalte instead
        else:
            normalized_card_names.add(card_name)
    # create set metadata
    set_file = MSEDataFile()
    set_file['mse version'] = '0.3.8'
    set_file['game'] = 'magic'
    set_file['stylesheet'] = 'm15-extra'
    set_info = {
        'title': 'MTG JSON card import',
        'copyright': args.copyright,
        'description': '{} automatically imported from MTG JSON using json-to-mse.'.format('This card was' if len(normalized_card_names) == 1 else 'These cards were'),
        'set code': args.set_code,
        'set language': 'EN',
        'mark errors': 'no',
        'automatic reminder text': '',
        'automatic card numbers': 'yes' if args.auto_card_numbers else 'no'
    }
    set_info['mana cost sorting'] = 'unsorted'
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
        'automatic card numbers': 'yes' if args.auto_card_numbers else 'no'
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
    schemes_set_file = MSEDataFile()
    schemes_set_file['mse version'] = '0.3.8'
    schemes_set_file['game'] = 'archenemy'
    schemes_set_file['stylesheet'] = 'standard' #TODO add option to use the E01 template when available
    schemes_set_file['set info'] = {
        'title': 'MTG JSON card import: Archenemy schemes',
        'copyright': args.copyright,
        'description': '{} automatically imported from MTG JSON using json-to-mse.'.format('This card was' if len(normalized_card_names) == 1 else 'These cards were'),
        'set code': args.set_code,
        'set language': 'EN',
        'mark errors': 'no',
        'automatic reminder text': '',
        'automatic card numbers': 'yes' if args.auto_card_numbers else 'no'
    }
    schemes_set_file['styling'] = { # styling needs to be above cards
        'archenemy-standard': {
            'text box mana symbols': 'magic-mana-small.mse-symbol-font',
            'tap symbol': 'modern'
        }
    }
    vanguards_set_file = MSEDataFile()
    vanguards_set_file['mse version'] = '0.3.8'
    vanguards_set_file['game'] = 'vanguard'
    vanguards_set_file['stylesheet'] = 'standard'
    vanguards_set_info = {
        'title': 'MTG JSON card import: Vanguard avatars',
        'copyright': args.copyright,
        'description': '{} automatically imported from MTG JSON using json-to-mse.'.format('This card was' if len(normalized_card_names) == 1 else 'These cards were'),
        'set code': args.set_code,
        'set language': 'EN',
        'mark errors': 'no',
        'automatic reminder text': '',
        'automatic card numbers': 'yes' if args.auto_card_numbers else 'no'
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
                    set_file.add_card(card, db, images=args.images, new_wedge_order=args.new_wedge_order, allow_uncards=args.allow_uncards)
                planes_set_file.add_card(card, db, layout='planechase', images=args.images, new_wedge_order=args.new_wedge_order, allow_uncards=args.allow_uncards)
            elif 'Scheme' in card.types:
                if args.include_schemes:
                    set_file.add_card(card, db, images=args.images, new_wedge_order=args.new_wedge_order, allow_uncards=args.allow_uncards)
                schemes_set_file.add_card(card, db, layout='archenemy', images=args.images, new_wedge_order=args.new_wedge_order, allow_uncards=args.allow_uncards)
            elif 'Vanguard' in card.types:
                if args.include_vanguards:
                    set_file.add_card(card, db, images=args.images, new_wedge_order=args.new_wedge_order, allow_uncards=args.allow_uncards)
                vanguards_set_file.add_card(card, db, layout='vanguard', images=args.images, new_wedge_order=args.new_wedge_order, allow_uncards=args.allow_uncards)
            else:
                set_file.add_card(card, db, images=args.images, new_wedge_order=args.new_wedge_order, allow_uncards=args.allow_uncards)
        except UncardError as e:
            print(f'[ !! ] Failed to add card {card_name}        ', file=sys.stderr)
            print(f'[ !! ] Un-cards are not supported and will most likely render incorrectly. Re-run with --allow-uncards to generate them anyway.', file=sys.stderr)
        except Exception as e:
            if args.verbose:
                raise RuntimeError(f'Failed to add card {card_name}') from e
            else:
                print(f'[ !! ] Failed to add card {card_name}        ', file=sys.stderr)
                failed += 1
    if failed > 0:
        print(f'[ ** ] {failed} cards failed. Run again with --verbose for a detailed error message', file=sys.stderr)
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
            elif stylesheet in ('m15-aftermath'):
                styling['center text 1'] = styling['center text 2'] = 'short text only'
            elif stylesheet == 'm15-textless-land':
                del styling['text box mana symbols'] # stylesheet has no text box
            else:
                styling['center text'] = 'short text only'
            set_file['styling'][f'magic-{stylesheet}'] = styling
    # generate footers
    set_file['version control'] = planes_set_file['version control'] = {'type': 'none'}
    set_file['apprentice code'] = planes_set_file['apprentice code'] = ''
    # zip and write set files
    if args.verbose:
        print('[....] adding images and saving', end='\r', flush=True, file=sys.stderr)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'x') as f:
        f.writestr('set', str(set_file))
        for i, image_path in enumerate(set_file.images):
            f.write(image_path, arcname=f'image{i + 1}')
    if args.verbose:
        print('[=...]', end='\r', flush=True, file=sys.stderr)
    args.output.write(buf.getvalue())
    args.output.flush()
    if args.verbose:
        print('[==..]', end='\r', flush=True, file=sys.stderr)
    if args.planes_output is not None:
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'x') as f:
            f.writestr('set', str(planes_set_file))
            for i, image_path in enumerate(planes_set_file.images):
                f.write(image_path, arcname=f'image{i + 1}')
        args.planes_output.write(buf.getvalue())
        args.planes_output.flush()
    if args.verbose:
        print('[===.]', end='\r', flush=True, file=sys.stderr)
    if args.schemes_output is not None:
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'x') as f:
            f.writestr('set', str(schemes_set_file))
            for i, image_path in enumerate(schemes_set_file.images):
                f.write(image_path, arcname=f'image{i + 1}')
        args.schemes_output.write(buf.getvalue())
        args.schemes_output.flush()
    if args.verbose:
        print('[====]', end='\r', flush=True, file=sys.stderr)
    if args.vanguards_output is not None:
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, 'x') as f:
            f.writestr('set', str(vanguards_set_file))
            for i, image_path in enumerate(vanguards_set_file.images):
                f.write(image_path, arcname=f'image{i + 1}')
        args.vanguards_output.write(buf.getvalue())
        args.vanguards_output.flush()
    if args.verbose:
        print('[ ok ]', file=sys.stderr)

if __name__ == '__main__':
    main()
