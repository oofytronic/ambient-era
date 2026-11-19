# Spelling dictionary

`words.txt` is the list the window checks spelling against. One lowercase word per line, sorted, three letters or more, letters a to z only.

## Source

The Hunspell `en_US` and `en_GB-ise` dictionaries, version 2020.12.07, as published in the npm packages `dictionary-en` 4.0.0 and `dictionary-en-gb` 3.0.0. They are derived from SCOWL (size 60) by Kevin Atkinson. The terms are in [SCOWL-LICENSE.txt](SCOWL-LICENSE.txt). The notice has to stay with the list.

Both spellings are accepted, so `color` and `colour` are both fine. Words with an apostrophe are not in the list. The checker handles `'s`, `n't`, `'re`, `'ve`, `'ll`, `'d`, and `'m` in code.

## How `words.txt` was made

```
python3 unmunch.py path/to/dictionary-en      us.txt
python3 unmunch.py path/to/dictionary-en-gb   gb.txt
sort -u us.txt gb.txt > words.txt
```

`unmunch.py` applies each word's affix flags from `index.aff`, so `walk` gives `walks`, `walked`, `walking`, `walker`. The result is lowercased and kept only if it is ASCII letters, three or longer.

## `words-extra.txt`

Words the two dictionaries lack and a person writing notes on a computer would use: `email` forms, `sync`, `repo`, `async`, `markdown`, `passkey`, `plaintext`, and a few ordinary words that were missing (`snuck`, `donut`). Add a word here, not to `words.txt`, so the generated file can be rebuilt. Keep it lowercase, sorted, and without duplicates of `words.txt`. A test checks that.

This is a list of words, not a promise that a word is correct. A rare real word can hide a typo. That is the cost of a list this size.
