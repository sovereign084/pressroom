# PressRoom and Microsoft Publisher: what matches and what doesn't

PressRoom is meant to feel the same as Publisher to someone who has used Publisher for years and doesn't want to learn a new program. This document lists what matches, what was changed to match, and what's still different.

The typical job is a weekly church bulletin or newsletter: edit text, swap pictures, reorder pages, and print a folded booklet on a home or office printer. Gaps that block or slow that job rank first.

## How each claim was checked

| Mark | Source | Strength |
| --- | --- | --- |
| **MS** | Microsoft's article "Keyboard shortcuts in Publisher" (support.microsoft.com, article 289f9c39), read 2026-11-01 | Strong for keys. The article may be dated. |
| **TLB** | Names in Publisher's COM type library (`MSPUB.TLB`, on the dev PC), read without starting Publisher | Strong that a feature exists. Says nothing about its menu location or wording. |
| **CAP** | Read-only captures of running Publisher (scripts not committed) | Strong for what was captured. |
| **KNOW** | General knowledge of Publisher's interface | Weakest. Not checked on this PC, because driving Publisher's window by hand was declined. Any of these may be wrong in detail. |

Nothing is copied from Publisher: no icons, templates, clip art or wording beyond feature names.

This document was reviewed on 2026-11-01 by three fresh reviewers (scope, design, and accuracy against the code). Their findings reordered the work and are built into what follows.

## What PressRoom mirrors

### Window
- Title bar with Quick Access Toolbar; tabs File, Home, Insert, Page Design, Mailings, Review, View and Help; contextual tabs that follow the selection, with Design and Layout for tables. (CAP)
- Four Office themes plus Use system setting, and an imitation of the Windows 11 window material.
- Ribbon groups in Publisher's order, with ScreenTips. Groups fold into single buttons from the right when the window is narrow. (KNOW)
- A task pane at the right for Find and Replace, Mail Merge, Design Checker, Graphics Manager, Hyperlinks, Bookmarks, Symbols and Table Options. (KNOW)
- Pages pane: thumbnails, facing pages together, drag a thumbnail to move a page, right-click for page commands, page names on hover. (KNOW; TLB for names)
- Status bar: page count, the selected object's position and size (a click opens Format), view buttons, zoom slider, Fit Page to Window. (KNOW)

### Pointer and objects
- One pointer and no toolbox: the text cursor inside a text box, the four-way arrow on its border and on other objects, resize arrows on handles, a cross while Draw Text Box is ready. Single letters only type. (CAP, KNOW)
- A text box's border takes the box to move or delete it, also while typing. Esc while typing selects the box. (CAP)
- Steel-blue outline, round corner handles, square side handles, a rotate handle on a stem, all turning with a turned object. (CAP)
- Ctrl+drag copies. Ctrl+click adds to the selection. Shift keeps proportions on a handle, Ctrl grows from the centre. Alt suspends snapping. Arrow keys nudge, Alt+arrows nudge a little or nudge the box being typed in. (MS for nudging; KNOW for the rest)
- A click with a shape tool or Draw Text Box makes one at a default size, and the pointer returns to selecting after a shape is drawn. (KNOW)
- A picture's corner handles keep proportions and its side handles stretch it. Crop shows black marks and trims. (KNOW)
- In a group, the first click selects the group and the second reaches a text box inside it. (MS for Esc; KNOW)

### Text
- Font group: Windows font names with previews and recent fonts, Bold, Italic, Underline, sub- and superscript, Change Case, Character Spacing, Font Color. With nothing selected, font, size, Bold, Italic, Underline and colour wait for the next text box.
- Font and Paragraph dialogs (Indents and Spacing, Line and Paragraph Breaks, Tabs), Character Spacing, Hyphenation. (MS, TLB)
- Line spacing applies to whole paragraphs, as in Publisher. (TLB)
- Text Fit: Best Fit, Shrink Text On Overflow, Grow Text Box to Fit, Do Not Autofit. (TLB)
- Linked text boxes: Create Link, Break, Previous, Next; a mark on a box whose text doesn't fit, which starts a link when clicked; arrows on a linked box. Boundaries off doesn't hide the mark. (TLB, KNOW)
- Number Style, Ligatures, Small Caps, Drop Cap, Text Fill, Text Outline, Text Effects. (MS, TLB)
- Spelling: red underlines on by default, suggestions on right-click, F7 with Ignore, Ignore All, Change, Change All and Add. Words in other alphabets, web and e-mail addresses, and words joined to numbers aren't flagged. (MS, KNOW)
- Find and Replace in a pane, with Match whole word and Match case. (MS)
- Format Painter: a click paints once, a double-click keeps painting, Ctrl+Shift+C and V from the keyboard. (MS)

### Objects
- Shapes gallery: lines and arrows, rectangles, basic shapes, block arrows, stars and banners, callouts, flowchart shapes; Change Shape. Our own drawings. (TLB)
- Colour menus: No Fill, scheme colours, the publication's colours, More Colors; arrowheads under Shape Outline. (MS)
- Shape Effects and Picture Effects: Shadow, Glow, Soft Edges, Bevel. (MS)
- Size group (Height, Width) on the Format tabs, and a Format dialog with Colors and Lines, Size, Layout, Text Box and Alt Text. (KNOW)
- Wrap Text: None, Square, Tight, Top and Bottom. (TLB)
- WordArt, Page Parts, Calendars, Borders & Accents and Advertisements, all our own designs. Colour and font schemes, page background, header and footer. Page numbers by position; Date and Time with its formats. (TLB, KNOW)
- Right-click menus in Publisher's shape: Cut, Copy, Paste, Delete Object, Save as Picture, Change Picture, Group, Order, Format, Hyperlink; Font and Paragraph on text; Zoom and Ruler Guides on the page. (KNOW; TLB for Save as Picture)

### Pictures
- Pictures fill the selected box and are kept in the publication. Pasting a picture from the clipboard and dropping a file on a box both work. An empty picture box shows an icon that asks for a picture when clicked. (KNOW)
- Corrections, Recolor, Compress Pictures, Change Picture, Reset Picture, Picture Border, Picture Styles, Caption, Swap, Clear Crop, Fit, Fill. (TLB)

### Tables
- A table is its own object. A size grid under the Table button, Insert Table, Table Tools Design (Table Formats, Fill, Line Weight, Line Color, Borders) and Layout (Select, Delete, Insert, Merge, Split, Alignment). (TLB, KNOW)

### Pages
- Insert Page with its options, Insert Duplicate Page, Delete, Move, Rename, master pages (Ctrl+M), Two-Page Spread, Go To Page. (MS, TLB)

### Print and files
- Print page: printer and Printer Properties, copies, All / Current / Custom, One page per sheet, Multiple pages per sheet, Multiple copies per sheet, Booklet side-fold with blanks added to a multiple of 4, both-sided on either edge, Colour or Grayscale. (TLB)
- **Manually Print on Both Sides** for printers that can't turn the paper: fronts first, then a prompt to turn the stack over, with last-sheet-first and upside-down options. It's chosen for you when the printer says it can't print both sides. (TLB)
- Booklets can reprint one sheet. A notice names any page with text that doesn't fit, before paper is used.
- The preview draws the same list of printed sides the job prints.
- Create PDF, Save as Picture, Save as Web Page, Pack and Go. Design Checker on File › Info. File › Options. Automatic recovery, self-update.
- `.pub` files convert with Publisher's own reading of the file. Reopening a converted `.pub` opens the copy with its edits instead of making a second one.

### Keys (MS)
Everything in Microsoft's table that applies, matched exactly (Ctrl+Shift+F is not taken for Ctrl+F): Ctrl+B, I, U, E, L, R, J, Z, Y, S, P, N, O, F, H, A, M, G, K, W, F4, F7, PageUp, PageDown; Ctrl+[ and ]; Ctrl+= and Ctrl+Shift+=; Ctrl+Shift+> and <, ] and [, C, D, F, G, H, K, L, N, O, S, U, V, X, Y, 0; Ctrl+Space; Alt+F6, Alt+Shift+F6, Alt+Backspace; Alt+Shift+T, D, P; F3, F4, F5, F7, F9; Shift+F4. Keys inherited from a page-layout program are off in PressRoom's window.

## What's still different

| Item | Source | Why it's left |
| --- | --- | --- |
| Print settings aren't saved with the publication | KNOW | Needs a place in the file or settings. Next on the list. |
| Top-fold booklet, half-fold, quarter-fold, tiled printing | TLB | Rare for this job. The sheet plan makes them straightforward to add. |
| "Continued on page" notes | TLB | Needs a live field in text. Only worth it if real files use it. |
| Autoflow: PressRoom says text doesn't fit and points to the mark; Publisher offers to flow it | KNOW | The manual route works. |
| View › Fields and Scratch Area | KNOW | Low use. |
| Set Transparent Color on a picture | TLB | Needs per-picture keying in the renderer. |
| Custom Drop Cap dialog; Text Direction | KNOW, TLB | Low use. |
| Bullets and Numbering, New Style and Replace Fonts open inherited dialogs | KNOW | They work; their wording isn't Publisher's. |
| IDML and PowerPoint export of Text Fit | | Those exports write the story's own sizes, unscaled. PDF and print are right. |

### Left out on purpose

| Feature | Why |
| --- | --- |
| Online Pictures, Research, Translate | Need online services and accounts. |
| Thesaurus | Needs a word list that isn't shipped. A free one could be added, but that's a download decision. |
| E-mail Merge, Send as Message | Depend on Outlook. |
| Object (embedding from other programs), Macros, Visual Basic | Windows-only automation a bulletin doesn't need, and a security risk. |
| Reflection and 3-D effects | The renderer can't draw them. A partial look would be worse than none. |
| BorderArt | Publisher's art can't be copied. Borders & Accents covers the common need with our own designs. |
| Business Information sets, Catalog Merge | Rarely used in this job. |
| East Asian features and web mode | Not used by this audience. |

## Risks

- **KNOW items can be wrong.** They rest on memory of Publisher. The changes are limited to PressRoom's window, so each can be corrected without touching DesignCraft's behaviour. A person who knows Publisher well should try them.
- **Nobody has used this on a real week's work yet.** The tests and the reviews can't replace watching a bulletin get made and printed on a real home printer, especially manual two-sided printing, whose order depends on the printer's paper path.
- **The dictionary** comes from a hyphenation list, so it misses some short and recent words. The common ones found are added by hand; others can be added to the dictionary as they're met.
- **Conversion needs Publisher**, which Microsoft is retiring. Old files should be converted while a copy of Publisher still runs.

## Design decisions and the alternatives set aside

- **Text Fit lives in the engine as a scale on the box**, composed over the story's own sizes. Rewriting font sizes from the interface was rejected: it breaks undo, runs only in the window, destroys the original sizes, and can't grow text back.
- **Shapes are a named preset on `frame.create`**, with geometry in `geom/shapes.rs`, so the drag previews the real shape. Swapping a rectangle's path when the drag ends was rejected: it takes two undo steps and misses clicks.
- **The print job and the preview read one list of sides** (`designcraft_pdf::SheetSide`). A separate imposition for each layout, and the preview's own fold arithmetic, were rejected: they can disagree with the paper.
- **Three kinds of PressRoom-only code.** What saves or prints is additive engine code with no flag, so a file lays out the same everywhere. Gestures use `ToolContext.publisher`. Menus, dialogs and the window's chrome are separate code in `crates/ui-egui/src/pressroom/`, reached by one line from DesignCraft's code, so upstream changes keep merging.
