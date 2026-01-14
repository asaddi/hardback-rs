# hardback

## What is hardback?

hardback is a simple program to archive/backup arbitrary data of a few kilobytes to paper. It is inspired by the likes of [PaperBack](http://www.ollydbg.de/Paperbak/index.html), [Paperkey](http://www.jabberwocky.com/software/paperkey/), and [optar](http://ronja.twibright.com/optar/).

I wanted something different because:

* I wanted to archive/backup any kind of data (not just GnuPG keys, as in Paperkey's case).
* If, in the future, I didn't have a scanner, I wanted to be able to fall back to "manual OCR," so-to-speak (ruling out PaperBack and optar).
* I was only interested in archiving small files (a few kilobytes at the most).

hardback encodes raw data using a straightforward base-32 conversion. The base-32 alphabet used is [z-base-32](https://philzimmermann.com/docs/human-oriented-base-32-encoding.txt), which is made up of of all numbers and lowercase letters, omitting '0', 'l', 'v', and '2'.

Each line is protected by a CRC-20 code run over the current line and all previous lines. The CRC is encoded as 4 base-32 characters at the end of each line.

Finally, I have done some basic round-trip testing:

1. Encode with hardback.
2. Print using a monospace font. I chose [Inconsolata](https://levien.com/type/myfonts/inconsolata.html), which is open-source and freely available.
3. Scan to JPEG at 300 dpi.
4. Use OCR to get it back to a text file. I used two methods. Both were successful, however both had their own set of quirks.
   * [GOCR](http://jocr.sourceforge.net/) using default settings. It tended to insert spaces where there were none. Also, unreadable characters were written out as an underscore '_', which I had to fill in manually.
   * The "Preview" app on iOS/iPadOS. It mistook every 'o' for '0'. It also inserted some Unicode/Cyrillic characters, which were a pain to find.
5. (Note that hardback outputs a "comment" at the end which includes the original file length, SHA-256 hash, as well as hints on how to decode everything. These should be edited out as they are not used.)
6. Iteratively ran `hardback -d` until it decoded successfully. Every time it errored out, I edited the text file and cleaned up the line in question, comparing it to the scanned image using "manual OCR" (aka my eyes).

I imagine using a higher resolution scan and/or better OCR software would cut down on the trial-and-error.

Also, each line encodes 50 bytes (not including the CRC), so expect about ~3000 bytes per page, assuming 60-65 lines per page.

## Usage

To encode an arbitrary file `[input-file]`:

    hardback [input-file]

The output (which is always plain text) is sent to stdout. Alternatively:

    hardback -o [output-file] [input-file]

If you would like to write the encoded output to a file.

To decode a file `[input-file]`:

    hardback -d [input-file]

The output, which is potentially binary data, will be sent to stdout. Alternatively:

    hardback -d -o [output-file] [input-file]

To write the decoded output to a file.

To re-iterate what was mentioned previously, when decoding:

* The `[input-file]` should be a plain text (ASCII, not Unicode) file outputted by your OCR program.
* Each line should be 84 characters long, except for maybe the last.
* The "comment" lines that hardback outputs at the end (they begin with '#') should be omitted.

## License

Licensed under the Apache License, Version 2.0. See https://www.apache.org/licenses/LICENSE-2.0.html or the included LICENSE file.

## Contact

I offer no support, but if you have any comments or find a bug, please feel free to contact me at <allan@saddi.com>.
