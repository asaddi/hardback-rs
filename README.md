hardback
========

**Note:** This is a Rust rewrite of the original Python version. Just more Rust practice. It is just a toy (both are, actually). Do not use!

What is hardback?
-----------------
hardback is a simple paper archive/backup program inspired by the likes of
[PaperBack](http://www.ollydbg.de/Paperbak/index.html), [Paperkey](http://www.jabberwocky.com/software/paperkey/), and [optar](http://ronja.twibright.com/optar/).

I wanted something different because:

* I wanted to archive/backup any kind of data (not just GnuPG keys,
  in Paperkey's case).
* If, in the future, I didn't have a scanner, I wanted to be able to fall
  back to "manual OCR," so-to-speak (ruling out PaperBack and optar).
* I was only interested in archiving small files (1-2 kilobytes).

hardback's encoding scheme is quite straightforward. It uses a base-32
encoding to encode raw data. The base-32 alphabet is the same as
[z-base-32](https://philzimmermann.com/docs/human-oriented-base-32-encoding.txt) and thus is made up of all numbers and lowercase
letters, omitting '0', 'l', 'v', and '2'.

Each line is protected with a CRC-20 run over the current line and all
previous lines. The CRC is encoded as 4 base-32 characters at the end of
the line. The SHA-256 hashe of the overall file are also part of
the output, though it is not used by the program when decoding.

Finally, the only round-trip testing I've done is as follows:

  hardback -> enscript -> PostScript -> JPEG -> [GOCR](http://jocr.sourceforge.net/) -> hardback

In other words, I have not actually tried scanning a printed document and
decoding the result! (Mainly because I don't own a functioning scanner!)
Rely on it at your own risk!

However, I was pretty satisfied with hardback's ability to decode a
GOCR-read image (originally "printed" with enscript, using a 10pt Courier
font). It's good enough for my needs and I've since used it to backup some
GnuPG keys and other files.

Usage
-----

**Todo:** Doesn't actually reflect the Rust version.

By default, hardback will encode files. If a file isn't specified as an
argument, it will read from stdin. Additional options:

-o OUTFILE     Send the output to OUTFILE instead of stdout

-d DECODE_LEN  Attempt to decode the input. DECODE_LEN is the length of the
               original file.

License
-------
Licensed under the Apache License, Version 2.0. See
http://www.apache.org/licenses/LICENSE-2.0.html or the included LICENSE file.

Contact
-------
I offer no support, but if you have any comments or find a bug, please
feel free to contact me at <allan@saddi.com>.
