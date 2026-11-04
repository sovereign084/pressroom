# The Publisher file format, as PressRoom reads it

What `crates/pubread` knows about `.pub` files from Publisher 2002 on. Microsoft publishes no
description of this format. These facts come from reading real files against what Publisher itself
reports about them (`tools/pub-import/pub2pressroom.ps1` drives Publisher's own object model, and its
step list is the check), and from the facts documented by libmspub, the LibreOffice project's reader.
libmspub's files carry MPL-2.0 headers (one, `ShapeFlags.h`, a tri-licence of MPL 1.1, GPLv2+ and
LGPLv2+). None of its code is used or translated here. The container and the drawing stream are
documented by Microsoft as [MS-CFB] and [MS-ODRAW].

A `.pub` is a compound file holding four streams that matter:

| Stream | Holds |
|---|---|
| `Contents` | Document size, pages, which shapes are on each, which story each shape shows, tables |
| `Quill/QuillSub/CONTENTS` | All the text, its formatting, the style sheet, font names, colour list |
| `Escher/EscherStm` | Each shape's type, place, fill, line and picture number, back to front |
| `Escher/EscherDelayStm` | The pictures themselves |

## Contents

Starts `E8 AC 2C 00` (Publisher 2002 and later; `E8 AC 22 00` is Publisher 2000, not read yet). The u32
at `0x1A` points to a trailer: a u32 length, then records, one of kind `0x90` being the directory. Its
children are numbered from 0, dummies included, and that number is the "sequence number" everything
else refers to. A child holds the chunk's kind (id 2), offset (id 4) and parent (id 5).

Records are an id byte and a kind byte. The kind says how long the value is: `0x10 0x12 0x18 0x1A` two
bytes, `0x20 0x22 0x58 0x68 0x70 0xB8` four, `0x28` eight, `0x38` sixteen, `0x48` twenty-four; `0x80 0x82
0x88 0x8A 0x90 0x98 0xA0 0xC0` hold a u32 length (counting itself) and then more records, or for `0xC0` a
string; any other kind is a flag with no value, its being there being what counts.

| Chunk kind | What | Ids |
|---|---|---|
| `0x44` document | | `0x12` size: 1 width, 2 height (EMU); `0x02` page list; `0x06` flag: two-page view (seen on one file) |
| `0x43` page | | `0x02` shape list; `0x0D` its master; `0x0E` name, non-empty on a master |
| `0x01 0x20 0x30 0x31` shape, group | | `0x27` story; `0x28` place in a chain of linked boxes (absent is first); `0x35` vertical alignment; `0x04` text wrap: `0x103` none, absent wraps (other values not yet seen) |
| `0x10` table | | `0x66` rows, `0x67` columns, `0x6B` cells chunk, `0x6D` list whose entries' id 2 are column widths then row heights (EMU), `0x27` story |
| `0x63` cells | | `0x02` list in **story order**, which needn't be along the rows: 1, 2 rows; 3, 4 columns; 7 vertical alignment; `0x0A` left, `0x0B` right, `0x0C` top, `0x0D` bottom margins (EMU). An id left out is 0 |
| `0x5C` palette | | list: id 1 is a scheme colour, `0x00BBGGRR` |

Pages `0x10D 0x110 0x113 0x117` are Publisher's own working pages. Linked boxes share a story.

## Text

A list at `0x18` of 24-byte entries names sections: `TEXT` (UTF-16, stories one after another), `SYID`
(story ids), `STRS` (their lengths in units), `FONT`, `PL  ` (colours), `BTEC`/`BTEP` (the formatting
pages for characters and paragraphs), `FDPC`/`FDPP` (those pages, 512 bytes each), `STSH` (the second is
the style sheet: entries in pairs, characters then paragraph, by text style) and `TCD ` (cell ends, in
units, by story's place in the list).

A formatting page: u16 count, a u32 start at +4, then ends (u32, absolute in the stream) and offsets
(u16) of the style records. Formatting a run doesn't say falls back to its paragraph's text style.

| Character id | | Paragraph id | |
|---|---|---|---|
| `0x02` flag | bold | `0x04` | alignment: 0 left, 1 right, 2 centre, 3 and 6 justified, 4 distributed |
| `0x03` flag | italic | `0x19` | text style |
| `0x0C` | size, EMU | `0x34` | line spacing: bit 0 set, points = (v-1) / 8 EMU; bit 1 set, lines = (v-2) / (12700 x 96) |
| `0x13` flag, `0x14` flag | small caps, all caps | `0x12`, `0x13` | space before, after, EMU |
| `0x1B` | spacing between letters, EMU | `0x0C`, `0x0D`, `0x0E` | first, left, right indents, EMU |
| `0x1E` | underline | `0x08`, `0x2D` | drop cap lines, letters (absent is 1) |
| `0x1F` | tracking, tenths of a percent | `0x53`, `0x56` | drop cap's font index, and colour (its id 0) |
| `0x20` | scale, tenths of a percent | `0x18` flag | keep with next |
| `0x24` | fonts by script slot: slot 0 ordinary letters, else 5 | `0x1D` flag | widow control |
| `0x44` | colour index (its id 0) | | |
| `0x00` | field: 5 is a page number, the text holding `#` | | |
| `0x12` | language | | |

`\r` ends a paragraph, `\v` is a line break. A colour-matching ink's description is in a colour's entry
and on the shape (tertiary property `0x1A1`): `P2,#002c005e004f0000,PANTONE 626 C`. The name is there and
the screen colour; the recipe for the press is not.

## Drawing

Records as [MS-ODRAW], except that the drawing and drawing-group containers are followed by a four byte
trailer. A shape's client anchor is pairs of (u16 id, i32 value) after a u32 length, in EMU measured
**from the middle of the page**: `0x2001`-`0x2004` left, top, right, bottom, a zero value left out. Its
client data's `0x6801` is its sequence number. A shape turned nearer a quarter than not stores its box
with the sides swapped about its middle.

Types as in [MS-ODRAW] (1 rectangle, 2 rounded rectangle, 3 ellipse, 20 line, 32 connector, 202 text box),
with 201 a table. A shape's crop (`0x100`-`0x103`) of 255 or more marks a leftover Publisher never draws.

A table's cell fills and border lines are drawing records with no box of their own: the anchor has
`0x6802`, the table's number, and they take no sequence number. A fill has only `0x2003`, the cell's place
in the cells chunk. A line has `0x2004`-`0x2007`: the row and column it runs from and to on the grid's
lines, colour `0x181`, weight `0x1CB`.

The picture store's `FBSE` entries give each picture's offset in the delay stream, at +28. A metafile's
record gives its box and size in the 34 bytes after its id, and a crop is measured against those, not the
metafile's own tighter bounds.

## Not known yet

Other wrap modes and wrap distances, keep lines together, whether rows grow with their text, superscript
values, a page's background shape, Publisher 2000 and earlier. Setting these through Publisher's object
model on a test file hung it on 2026-11-03 with the printer offline, so they haven't been checked.
