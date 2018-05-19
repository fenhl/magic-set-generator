`json-to-mse` is a command-line tool which converts [MTG JSON](https://mtgjson.com/) card data into [Magic Set Editor](http://magicseteditor.sourceforge.net/) set files.

# Usage

Since MSE is a Windows programm, this guide assumes that you're running `json-to-mse` on Windows. For other platforms, adjust accordingly.

## Installation

* You will need Python 3.6 or later. [Download here](https://www.python.org/downloads/windows/).
    * Make sure to check the “Add Python to PATH” option when installing.
    * You can also install Python using [Chocolatey](https://chocolatey.org/). This is the recommended way to install Python if you already have Chocolatey on your system. If you do, run the following commands using `pip` or `python` instead of `pip3` or `python3`.
* Download [this zip file](https://github.com/fenhl/json-to-mse/archive/master.zip).
* Unzip the downloaded file, and open the resulting folder in the command line. To do so, right-click it in File Explorer while holding shift, then select “Open PowerShell window here” or “Open command prompt here”.
* In the command line, run the following command:
    ```
    pip3 install Pillow more-itertools mtgjson regex requests
    ```
* Some features require the Custom Magic template pack. To install, join [the Custom Magic Discord server](https://discord.gg/FbMK9UE) and follow the instructions in the message pinned in #resources. (Download the Full MTG pack, not the Basic M15 pack or the M15 pack.)

## Basic usage

* Open the downloaded folder in the command line. To do so, right-click it in File Explorer while holding shift, then select “Open PowerShell window here” or “Open command prompt here”.
* In the command line, run the following command:
    ```
    python3 json-to-mse.py Counterspell "Dryad Arbor" -o example.mse-set
    ```

    This will create an MSE set file containing the cards [Counterspell](https://mtg.wtf/card/ema/43) and [Dryad Arbor](https://mtg.wtf/card/v12/5) and save it as `example.mse-set` in the downloaded folder. (Note that card names containing spaces must be enclosed in quotation marks.)

    You can also save your card names as a plain text file in the downloaded folder (one card name per line), and use that file to generate the cards, like this: (let's assume the text file is called `cards.txt`)

    ```
    python3 json-to-mse.py -i cards.txt -o example.mse-set
    ```

## Advanced usage

The script takes any number of command line arguments. Arguments are interpreted as follows:

* Arguments starting with a `-` are interpreted as options (see below).
* Arguments starting with `!` are special commands. The following commands are currently supported:
    * `!all`: Generate all cards present in MTG JSON, except tokens and un-cards.
    * `!tappedout <deck-id>`: Download the given decklist from [tappedout.net](http://tappedout.net/) and generate all cards from it.
* Arguments starting with `#` are ignored. This can be used in input files (see `-i` below) to write comments.
* Arguments starting with `=` are parsed according to [mtg.wtf syntax](https://mtg.wtf/help/syntax) to generate all cards from the result. This requires the `find_cards` script from [magic-search-engine](https://github.com/taw/magic-search-engine), see also `--find-cards` below.
* Any other arguments are interpreted as card names. This can be used to specify cards to generate instead of, or in addition to, those read from an input file.

If your shell supports input/output redirection, you can also pipe card names into the script (again, one name per line), and pipe the output into a `.zip` file. For example,

```
echo 'Dryad Arbor' | python3 json-to-mse.py > example.mse-set
```

is equivalent to

```
python3 json-to-mse.py 'Dryad Arbor' -o example.mse-set
```

## Command-line options

`json-to-mse.py` accepts the following command line options:

* `-b`, `--border=<color>`: Set the card border color. Supported colors are:
    * `black`, the default
    * `w` or `white`
    * `s` or `silver`
    * `g` or `gold`
    * `b` or `bronze`, for clearly marking cards as proxies
* `-i`, `--input=<path>`: Read card names from the file or directory located at `<path>`. This can be specified multiple times to combine multiple input paths into one MSE set file. The following formats are understood:
    * A plain text file with one card name per line. Special lines are also supported as with directly specified arguments (see “advanced usage” above).
    * A directory containing images named `<card name>.png`. This will set `--images` to this directory if it's not already set (see below), and generate the named cards.
* `-o`, `--output=<path>`: Write the zipped MSE set file to the specified path, instead of the standard output.
* `-v`, `--verbose`: Report progress while generating the set file, and give more detailed error messages if anything goes wrong.
* `--allow-uncards`: This script has no official support for silver-bordered “un-cards” and other shenanigans like [1996 World Champion](https://mtg.wtf/card/uqc/1). As a result, most un-cards will be redered incorrectly, so the script will refuse to generate them unless this option is used. Reports of issues encountered while using this option will be closed as invalid.
* `--auto-card-numbers`: Display automatically-assigned collector numbers on the cards, below the text box.
* `--copyright=<message>`: The copyright message, appearing in the lower right of the card frame. Defaults to `NOT FOR SALE`.
* `--find-cards=<path>`: The path to the `find_cards` executable used for [mtg.wtf syntax](https://mtg.wtf/help/syntax). Defaults to `git\github.com\taw\magic-search-engine\master\search-engine\bin\find_cards`.
* `--images=<path>`: The path to a directory containing card art to use. Files should be named `<path>\<card name>.png`. By default, the generated set file does not include any images.
* `--[no-]include-planes`: Enable or disable the inclusion of planes and phenomena as regular-sized cards in the main set file. This is on by default unless `--planes-output` is given.
* `--[no-]include-vanguards`: Enable or disable the inclusion of vanguards as regular-sized cards in the main set file. This is on by default unless `--vanguards-output` is given.
* `--new-wedge-order`: In mana costs, order all three-color wedges using the new order (e.g. `WBG`), even if Oracle still uses the old one (e.g. `BGW`). By default, Oracle order is used.
* `--planes-output=<path>`: Save planes and phenomena to a separate MSE set file at the specified path. By default, these cards are not rendered using the correct oversized template, use this option to fix this.
* `--set-code=<code>`: The set code of the generated set. Defaults to `PROXY`.
* `--vanguards-output=<path>`: Save vanguards to a separate MSE set file at the specified path. By default, these cards are not rendered using the correct oversized template, use this option to fix this.
