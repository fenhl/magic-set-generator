`json-to-mse` is a command-line tool which converts [MTG JSON](https://mtgjson.com/) card data into [Magic Set Editor](http://magicseteditor.sourceforge.net/) set files.

# Usage

**Note:** This is an alpha preview of `json-to-mse` version 2. It is not yet feature complete. If you encounter any bugs or missing features, please [open an issue](https://github.com/fenhl/json-to-mse/issues/new) or let me know on Discord.

## Installation

1. Install Rust:
    * On Windows, download and run [rustup-init.exe](https://win.rustup.rs/) and follow its instructions.
    * On other platforms, please see [the Rust website](https://www.rust-lang.org/learn/get-started) for instructions.
2. Open a command line:
    * On Windows, right-click the start button, then click “Windows PowerShell” or “Command Prompt”.
    * On other platforms, look for an app named “Terminal” or similar.
3. In the command line, run the following command. Depending on your computer, this may take a while. You can continue with step 4 while it's running.
    ```
    cargo install --git=https://github.com/fenhl/json-to-mse --branch=riir
    ```
4. Some features may require MSE templates not packaged with MSE. You can get them from one of the following sources:
    * Cajun's Megafile (recommended):
        1. If you don't have MSE yet, download the Advanced Magic Set Editor files from <http://magicseteditor.boards.net/page/downloads>.
        2. Download the Megafile from <http://magicseteditor.boards.net/thread/77/cajun-templates-updates-sorting-update>.
        3. Merge the Megafile contents into your MSE `data` folder.
    * The Custom Magic template pack. To install, join [the Custom Magic Discord server](https://discord.gg/FbMK9UE) and follow the instructions in the message pinned in #resources. (Download the Full MTG pack, not the Basic M15 pack or the M15 pack.)

## Basic usage

1. Open a command-line in the folder where you want to save your MSE set file:
    * On Windows, locate the folder in File Explorer, then right-click it while holding shift and select “Open PowerShell window here” or “Open command prompt here”.
    * On other platforms, open a command line and navigate to the folder using `cd`. For example, if you want to save in your user folder → games → magic → sets, run the command `cd games/magic/sets`.
2. In the command line, run the following command:
    ```
    json-to-mse Counterspell "Dryad Arbor" -o example.mse-set
    ```

    This will create an MSE set file containing the cards [Counterspell](https://lore-seeker.cards/card/ss1/4) and [Dryad Arbor](https://lore-seeker.cards/card/fut/174) and save it as `example.mse-set` in the folder you selected. (Note that card names containing spaces must be enclosed in quotation marks.)

    You can also save your card names as a plain text file in the same folder (one card name per line), and use that file to generate the cards, like this: (let's assume the text file is called `cards.txt`)

    ```
    json-to-mse -i cards.txt -o example.mse-set
    ```

## Advanced usage

Sections marked **(NYI)** are not yet implemented in `json-to-mse` version 2.

The script takes any number of command line arguments. Arguments are interpreted as follows:

* Arguments starting with a `-` are interpreted as options (see below).
* Arguments starting with `!` are special commands. The following commands are currently supported:
    * `!all`: Generate all cards present in the database (see `--db` below), except tokens and un-cards.
    * **(NYI)** `!tappedout <deck-id>`: Download the given decklist from [tappedout.net](http://tappedout.net/) and generate all cards from it.
* Arguments starting with `#` are ignored. This can be used in input files (see `-i` below) to write comments.
* **(NYI)** Arguments starting with `=` are parsed according to [Lore Seeker syntax](https://lore-seeker.cards/help/syntax) to generate all cards from the result. This requires an internet connection or a `find_cards` script compatible with the one from [magic-search-engine](https://github.com/taw/magic-search-engine), see also `--find-cards` and `--offline` below.
* Any other arguments are interpreted as card names. This can be used to specify cards to generate instead of, or in addition to, those read from an input file.

If your shell supports input/output redirection, you can also pipe arguments into the script (again, one argument per line, and currently not supported on Windows), and pipe the output into a `.zip` file. For example,

```
echo 'Dryad Arbor' | json-to-mse > example.mse-set
```

is equivalent to

```
json-to-mse 'Dryad Arbor' -o example.mse-set
```

## Image handling

How card artwork is handled is determined as follows:

1. If `--no-images` is set, all artwork is left blank. Skip all following steps.
2. If `--images` is set to a directory containing a file named `<card name>.png`, that image will be used.
3. If `--scryfall-images` is set to a directory containing a file named `<card name>.png`, that image will be used.
4. If `--lore-seeker-images` is set to a directory containing a file named `<card name>.png`, that image will be used.
5. If neither `--no-scryfall-images` nor `--offline` are set, `json-to-mse` will attempt to download the card artwork from [Scryfall](https://scryfall.com/). If successful, that image is used. If `--scryfall-images` is set to a directory, the image will also saved there as `<card name>.png`. Otherwise, `json-to-mse` will attempt to save the image to `--images`, or simply discard it if that isn't set either.
6. **(NYI)** If neither `--no-lore-seeker-images` nor `--offline` are set, `json-to-mse` will attempt to download the card artwork from [Lore Seeker](https://lore-seeker.cards/). If successful, that image is used. If `--lore-seeker-images` is set to a directory, the image will also saved there as `<card name>.png`. Otherwise, `json-to-mse` will attempt to save the image to `--images`, or simply discard it if that isn't set either.
7. If none of the previous steps were successful, the artwork for that card is left blank.

## Command-line options

`json-to-mse` accepts the following command line options:

* `-b`, `--border=<color>`: Set the card border color. Supported colors are:
    * `b` or `black`
    * `w` or `white`
    * `s` or `silver`
    * `g` or `gold`
    * `bronze`, the default, for clearly marking cards as proxies
    * a 3- or 6-digit [hex triplet](https://en.wikipedia.org/wiki/Web_colors#Hex_triplet)
* `-h`, `--help`: Print a short message with a link to this readme file instead of doing anything else.
* `-i`, `--input=<path>`: Read card names from the file or directory located at `<path>`. This can be specified multiple times to combine multiple input paths into one MSE set file. The following formats are understood:
    * A plain text file with one card name per line. Special lines are also supported as with directly specified arguments (see “advanced usage” above). `!` commands and their arguments should be on the same line, with arguments shell-quoted if necessary.
    * **(NYI)** A directory containing images named `<card name>.png`. This will set `--images` to this directory if it's not already set (see below), and generate the named cards.
* `-o`, `--output=<path>`: Write the zipped MSE set file to the specified path, instead of the standard output.
* `-v`, `--verbose`: Check for self-updates (unless `--offline` is given), report progress while generating the set file, and give more detailed error messages if anything goes wrong.
* **(NYI)** `--allow-uncards`: This script has no official support for silver-bordered “un-cards” and other shenanigans like [1996 World Champion](https://lore-seeker.cards/card/pcel/1). As a result, most un-cards will be redered incorrectly, so the script will refuse to generate them unless this option is used. Reports of issues encountered while using this option will be closed as invalid.
* **(NYI)** `--auto-card-numbers`: Display automatically-assigned collector numbers on the cards, below the text box.
* **(NYI)** `--copyright=<message>`: The copyright message, appearing in the lower right of the card frame. Defaults to `NOT FOR SALE`.
* `--db=<path>`: The path from which to load the card database. In `--offline` mode, this defaults to `data\sets` in the [gitdir](https://github.com/fenhl/gitdir) master for [Lore Seeker](https://github.com/fenhl/lore-seeker). Otherwise, the database is downloaded from [mtgjson.com](https://mtgjson.com/) by default. The following formats are understood:
    * A file in the [MTG JSON AllSets](https://mtgjson.com/files/all-sets/) format.
    * A directory containing [MTG JSON Individual Set](https://mtgjson.com/files/individual-set/) files.
* **(NYI)** `--find-cards=<path>`: The path to the `find_cards` executable used for [Lore Seeker syntax](https://lore-seeker.cards/help/syntax). In `--offline` mode, this defaults to `search-engine\bin\find_cards` in the [gitdir](https://github.com/fenhl/gitdir) master for [Lore Seeker](https://github.com/fenhl/lore-seeker). Otherwise, [the Lore Seeker website](https://lore-seeker.cards/) is used by default.
* `--holofoil-stamps`: Enable holofoil stamps on the bottom of text boxes of rare and mythic cards.
* `--[no-]images[=<path>]`: See [Image handling](#image-handling).
* `--[no-]include-schemes`: Enable or disable the inclusion of schemes as regular-sized cards in the main set file. This is on by default unless `--schemes-output` is given.
* `--[no-]include-vanguards`: Enable or disable the inclusion of vanguards as regular-sized cards in the main set file. This is on by default unless `--vanguards-output` is given.
* `--[no-]lore-seeker-images[=<path>]`: See [Image handling](#image-handling).
* `--offline`: This option has the following effects:
    * It enables `--no-lore-seeker-images` and `--no-scryfall-images`.
    * `json-to-mse` won't check for self-updates, even in `--verbose` mode.
    * It won't attempt to download the card database. Instead, if `--db` isn't given, it expects a local copy of [the Lore Seeker repository](https://github.com/fenhl/lore-seeker). See `--db` for details.
    * It won't attempt to use [Lore Seeker](https://lore-seeker.cards/) for syntax queries (arguments starting with `=`). Instead, `find_cards` is required if any queries are performed. See `--find-cards` for details.
* **(NYI)** `--plane-templates=<templates>`: Specify which templates to use for planes and, as a comma-separated list of any number of the following. The default is `plane`. If multiple templates are specified, each plane and phenomenon card will appear multiple times in the set file.
    * `large`: The default Planechase template.
    * `mini`: A smaller version of the Planechase template, same size as regular cards. Very small text.
    * `basic`: The default template for regular cards.
* `--schemes-output=<path>`: Save schemes to a separate MSE set file at the specified path. By default, these cards are not rendered using a correct oversized template, use this option to fix this.
* `--[no-]scryfall-images[=<path>]`: See [Image handling](#image-handling).
* `--set-code=<code>`: The set code of the generated set. Defaults to `PROXY`.
* `--update`: Attempt to update `json-to-mse` to the latest version instead of doing anything else.
* `--vanguards-output=<path>`: Save vanguards to a separate MSE set file at the specified path. By default, these cards are not rendered using the correct oversized template, use this option to fix this.
* `--version`: Print version information instead of doing anything else.
