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
        self.cards = []
        for arg in args:
            if arg.startswith('-'):
                if arg.startswith('--'):
                    if arg == '--verbose':
                        self.verbose = True
                    else:
                        raise ValueError(f'Unrecognized flag: {arg}')
                else:
                    for short_flag in arg[1:]:
                        if short_flag == 'v':
                            self.verbose = True
                        else:
                            raise ValueError(f'Unrecognized flag: -{short_flag}')
            else:
                self.cards.append(arg)

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
        card_names = [line.strip() for line in sys.stdin] + args.cards
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
    for card_name in card_names:
        card = mtg_json(verbose=args.verbose).cards_by_name[card_name]
        set_file.add('card', MSEDataFile.from_card(card))
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'x') as f:
        f.writestr('set', str(set_file))
    sys.stdout.buffer.write(buf.getvalue())
    sys.stdout.buffer.flush()
