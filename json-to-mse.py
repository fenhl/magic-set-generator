#!/usr/bin/env python3

import sys

import io
import mtgjson
import zipfile

CACHE = {
    'db': None,
    'db_x': None
}

class CommandLineArgs:
    def __init__(self, args=sys.argv[1:]):
        self.verbose = False
        self.cards = set()
        self.output = sys.stdout.buffer
        mode = None
        for arg in args:
            if mode == 'input':
                self.set_input(arg)
                mode = None
            elif mode == 'output':
                self.output = open(arg, 'wb')
                mode = None
            elif arg.startswith('-'):
                if arg.startswith('--'):
                    if arg == '--input':
                        mode = 'input'
                    elif arg.startswith('--input='):
                        self.set_input(arg[len('--input='):])
                    elif arg == '--output':
                        mode = 'output'
                    elif arg.startswith('--output='):
                        self.output = open(arg[len('--output='):], 'wb')
                    elif arg == '--verbose':
                        self.verbose = True
                    else:
                        raise ValueError(f'Unrecognized flag: {arg}')
                else:
                    for i, short_flag in enumerate(arg):
                        if i == 0:
                            continue
                        if short_flag == 'i':
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

    def set_input(self, input_filename):
        with open(input_filename) as f:
            for line in f:
                self.cards.add(line.strip())

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

    def __setitem__(self, key, value):
        for iter_key, iter_value in self.items:
            if iter_key == key:
                raise KeyError('Key exists')
        self.add(key, value)

    def __str__(self):
        return self.to_string()

    @classmethod
    def from_card(cls, card_info):
        raw_data = card_info._get_raw_data()
        result = cls()
        result['name'] = card_info.name
        if 'manaCost' in raw_data:
            result['casting cost'] = card_info.manaCost
        if 'supertypes' in raw_data:
            result['super type'] = f'<word-list-type>{" ".join(card_info.supertypes)} {" ".join(card_info.types)}</word-list-type>'
        else:
            result['super type'] = f'<word-list-type>{" ".join(card_info.types)}</word-list-type>'
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
    if sys.stdin.isatty():
        card_names = args.cards
    else:
        card_names = set(line.strip() for line in sys.stdin) | args.cards
    if len(card_names) == 0:
        sys.exit('[!!!!] missing card name')
    set_file = MSEDataFile()
    set_file['mse version'] = '0.3.8'
    set_file['game'] = 'magic'
    set_file['stylesheet'] = 'm15'
    set_file['set info'] = {
        'title': 'MTG JSON card import',
        'description': '{} automatically imported from MTG JSON using json-to-mse.'.format('This card was' if len(card_names) == 1 else 'These cards were'),
        'set language': 'EN'
    }
    for card_name in sorted(card_names):
        card = mtg_json(verbose=args.verbose).cards_by_name[card_name]
        set_file.add('card', MSEDataFile.from_card(card))
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'x') as f:
        f.writestr('set', str(set_file))
    args.output.write(buf.getvalue())
    args.output.flush()
