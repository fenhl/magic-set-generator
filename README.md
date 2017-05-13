`json-to-mse` is a command-line tool which converts [MTG JSON](https://mtgjson.com/) card data into [Magic Set Editor](http://magicseteditor.sourceforge.net/) set files.

# Usage

Since MSE is a Windows programm, this guide assumes that you're running `json-to-mse` on Windows. For other platforms, adjust accordingly.

## Installation

* You will need Python 3.6 or later. [Download here](https://www.python.org/downloads/windows/).
    * Make sure to check the “Add Python to PATH” option when installing.
* Download [this zip file](https://github.com/fenhl/json-to-mse/archive/master.zip).
* Unzip the downloaded file, and open the resulting folder in the command line. To do so, right-click it in Explorer while holding shift, then select “Open command prompt here”.
* In the command line, run the following command:
    ```
    pip3 install more-itertools mtgjson
    ```

## Basic usage

* Open the downloaded folder in the command line. To do so, right-click it in Explorer while holding shift, then select “Open command prompt here”.
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

The script takes any number of command line arguments. Arguments starting with a `-` are interpreted as options (see below). Any other arguments are interpreted as card names. This can be used to specify cards to generate instead of, or in addition to, those read from an input file.

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
* `-i`, `--input=<path>`: Read card names from the plain text file located at `<path>`. This can be specified multiple times to combine multiple input files into one MSE set file.
* `-o`, `--output=<path>`: Write the zipped MSE set file to the specified path, instead of the standard output.
* `-v`, `--verbose`: Report progress while generating the set file, and give more detailed error messages if anything goes wrong.
* `--copyright=<message>`: The copyright message, appearing in the lower right of the card frame. Defaults to `NOT FOR SALE`.
* `--old-wedge-order`: In mana costs, order three-color wedges using the old order (e.g. `BGW`) instead of the new one (e.g. `WBG`).
* `--set-code=<code>`: The set code of the generated set. Defaults to `PROXY`.
