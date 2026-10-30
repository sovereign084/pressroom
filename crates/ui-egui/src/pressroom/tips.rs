//! Office-style ScreenTips for the ribbon: pointing at a button shows a small card with its name
//! in bold, the shortcut that does the same, and a line on what it's for. The descriptions are our
//! own words.

use egui::{Response, RichText};

/// Show the ScreenTip of the ribbon button labelled `label` (a `\n` breaks a big button's label;
/// "(Ctrl+B)" at the end is its shortcut) when the pointer rests on `resp`.
pub(super) fn screentip(resp: Response, label: &str) -> Response {
    let (name, keys) = title(label);
    let keys = keys.or_else(|| shortcut(&name).map(str::to_string));
    let head = match keys {
        Some(k) => format!("{name} ({k})"),
        None => name.clone(),
    };
    let about = describe(&name);
    resp.on_hover_ui(move |ui| {
        ui.set_max_width(260.0);
        ui.label(RichText::new(head).font(crate::theme::semibold(12.0)));
        if let Some(about) = about {
            ui.add_space(1.0);
            ui.label(RichText::new(about).size(11.5));
        }
    })
}

/// The button's name on one line, and the shortcut its label ends with.
fn title(label: &str) -> (String, Option<String>) {
    // "Advertise-\nments" is one word broken over two lines.
    let one = label.replace("-\n", "").replace('\n', " ");
    if let Some(open) = one.rfind(" (")
        && one.ends_with(')')
    {
        let keys = one.get(open + 2..one.len() - 1).unwrap_or_default();
        if keys.starts_with("Ctrl") || keys.starts_with("Alt") || keys.starts_with('F') {
            return (one.get(..open).unwrap_or_default().to_string(), Some(keys.to_string()));
        }
    }
    (one, None)
}

/// PressRoom's own shortcut for a button whose label doesn't name one (`keys.rs`, and the
/// clipboard keys every Windows program has).
fn shortcut(name: &str) -> Option<&'static str> {
    Some(match name {
        "Copy" => "Ctrl+C",
        "Cut" => "Ctrl+X",
        "Paste" => "Ctrl+V",
        "Find" => "Ctrl+F",
        "Replace" => "Ctrl+H",
        "Group" | "Ungroup" => "Ctrl+Shift+G",
        "Bring Forward" => "Alt+F6",
        "Send Backward" => "Alt+Shift+F6",
        "Spelling" => "F7",
        "Page" => "Ctrl+Shift+N",
        "Whole Page" => "Ctrl+Shift+L",
        "Clear All Formatting" => "Ctrl+Space",
        "Master Page" => "Ctrl+M",
        "100%" => "F9",
        _ => return None,
    })
}

/// What the button is for, in a sentence.
fn describe(name: &str) -> Option<String> {
    if let Some(group) = name.strip_suffix(" Settings") {
        return Some(format!("See all the {} options in one window.", group.to_lowercase()));
    }
    let s = match name {
        // Home: Clipboard, Font, Paragraph.
        "Paste" => "Put what you copied or cut here.",
        "Cut" => "Take the selection out and keep it on the Clipboard, ready to paste.",
        "Copy" => "Keep a copy of the selection on the Clipboard, ready to paste.",
        "Format Painter" => "Copy the look of the selection, then click text or an object to give it the same look.",
        "Bold" => "Make the text bold.",
        "Italic" => "Slant the text.",
        "Underline" => "Draw a line under the text.",
        "Subscript" => "Make the text small and set it below the line, as in H\u{2082}O.",
        "Superscript" => "Make the text small and set it above the line, as in 10\u{b2}.",
        "Grow Font" => "Make the text a little bigger.",
        "Shrink Font" => "Make the text a little smaller.",
        "Clear All Formatting" => "Take the formatting off the text, leaving plain text.",
        "Change Case" => "Change the text to UPPERCASE, lowercase or other capitalisation.",
        "Character Spacing" => "Spread the letters out or draw them closer together.",
        "Font Color" => "Change the colour of the text.",
        "Bullets" => "Start a bulleted list.",
        "Numbering" => "Start a numbered list.",
        "Decrease Indent Position" => "Move the paragraph back towards the margin.",
        "Increase Indent Position" => "Move the paragraph further in from the margin.",
        "Align Text Left" => "Line the text up on the left.",
        "Center" => "Centre the text between the edges of the box.",
        "Align Text Right" => "Line the text up on the right.",
        "Justify" => "Line the text up on both sides, spreading the words out evenly.",
        "Distribute" => "Spread every line, the last one too, across the whole box.",
        "Line Spacing" => "Choose how much space goes between lines of text and between paragraphs.",
        "Special Characters" => "Show or hide the marks for spaces, tabs and paragraph ends.",
        "Styles" => "Give the text a ready-made look from the publication's styles.",
        // Objects and Arrange.
        "Draw Text Box" => "Draw a box on the page to type text into.",
        "Pictures" => "Put a picture from your computer on the page.",
        "Online Pictures" => "Find pictures on the web.",
        "Table" => "Add a table of rows and columns.",
        "Shapes" => "Draw a ready-made shape: lines, arrows, boxes, stars and more.",
        "Wrap Text" => "Choose how text flows around the selected object.",
        "Bring Forward" => "Move the object in front of the one above it.",
        "Send Backward" => "Move the object behind the one below it.",
        "Group" => "Join the selected objects so they move and size together.",
        "Ungroup" => "Separate a group back into its objects.",
        "Align" => "Line up the selected objects with each other or with the page.",
        "Rotate" => "Turn or flip the selected object.",
        "Find" => "Look for words in the publication.",
        "Replace" => "Find words and swap them for others.",
        "Select" => "Select objects or all the text in a box.",
        // Insert.
        "Page" => "Add a page to the publication.",
        "Picture Placeholder" => "Add an empty frame that holds a picture's place until you choose one.",
        "Page Parts" => "Add a ready-made heading, sidebar or quote.",
        "Calendars" => "Add a calendar for a month.",
        "Borders & Accents" => "Add a decorative border or accent.",
        "Advertisements" => "Add a ready-made advertisement block.",
        "WordArt" => "Add decorative text.",
        "Insert File" => "Bring the text of a file into a text box.",
        "Symbol" => "Insert a character that isn't on the keyboard, such as \u{a9} or \u{2122}.",
        "Date & Time" => "Insert today's date or the time.",
        "Object" => "Insert content made in another program.",
        "Link" => "Link the text to a web page or an email address.",
        "Bookmark" => "Mark a place in the publication that links can go to.",
        "Header" => "Edit the text at the top of every page.",
        "Footer" => "Edit the text at the bottom of every page.",
        "Page Number" => "Number the pages.",
        // Page Design.
        "Change Template" => "Start again from another built-in design.",
        "Margins" => "Choose the margins around each page.",
        "Orientation" => "Turn the pages between portrait and landscape.",
        "Size" => "Choose the page size.",
        "Guides" => "Show guides that help you line things up.",
        "Move" => "Move the page within the publication.",
        "Delete" => "Delete the page.",
        "Rename" => "Give the page a name.",
        "Master Pages" => "Edit the background pages that other pages are built on.",
        "Color Schemes" => "Change the publication's colours all at once.",
        "Fonts" => "Change the publication's fonts all at once.",
        "Background" => "Give the page a background colour or picture.",
        // Mailings, Review, View, Help.
        "Mail Merge" => "Put names and addresses from a list into copies of the publication.",
        "E-mail Merge" => "Send personalised copies of the publication by email.",
        "Spelling" => "Check the spelling in the publication.",
        "Research" => "Look things up in reference books and on the web.",
        "Thesaurus" => "Find other words that mean the same.",
        "Translate" => "Translate the text into another language.",
        "Language" => "Choose the language that spelling is checked in.",
        "Normal" => "Show the publication's pages as usual.",
        "Master Page" => "Show the background page that this page is built on.",
        "100%" => "Show the page at its real size.",
        "Whole Page" => "Fit the whole page in the window.",
        "Page Width" => "Fit the width of the page in the window.",
        "Selected Objects" => "Zoom in on the selected objects.",
        "Zoom" => "Choose how close to show the page.",
        "Single Page" => "Show one page at a time.",
        "Two-Page Spread" => "Show facing pages side by side, as they'll be printed.",
        "Switch Windows" => "Go to another open publication.",
        "Help" => "Get help with PressRoom.",
        "What's New" => "See what's new in this version of PressRoom.",
        "Feedback" => "Tell us what you like, or what could be better.",
        "Check for Updates" => "See if there's a newer PressRoom to install.",
        // Format tabs.
        "Text Fit" => "Shrink or grow the text so it fits its box.",
        "Columns" => "Split the text box into columns.",
        "Drop Cap" => "Make the paragraph's first letter large.",
        "Create Link" => "Let the text flow on from this box into another.",
        "Break" => "Stop the text flowing on into the next box.",
        "Next" => "Go to the next box the text flows into.",
        "Previous" => "Go to the box the text flows from.",
        "Fill" => "Colour the inside of the box.",
        "Shape Fill" => "Colour the inside of the shape.",
        "Shape Outline" => "Choose the colour and thickness of the shape's outline.",
        "Picture Border" => "Draw a border around the picture.",
        "Change Picture" => "Put a different picture in its place, keeping its size.",
        "Reset Picture" => "Undo the changes made to the picture.",
        "Crop" => "Trim the edges of the picture.",
        "Fit" => "Fit the picture to its frame.",
        "Corrections" => "Change the picture's brightness and contrast.",
        "Recolor" => "Give the picture a colour tint.",
        "Compress Pictures" => "Make the pictures smaller to save space.",
        "Merge Cells" => "Join the selected cells into one.",
        "Split Cells" => "Split a merged cell back apart.",
        "Insert Above" => "Add a row above.",
        "Insert Below" => "Add a row below.",
        "Insert Left" => "Add a column on the left.",
        "Insert Right" => "Add a column on the right.",
        "Table Options" => "Set the table's rows, columns and spacing.",
        "Align Top" => "Line the text up at the top of the cell.",
        "Align Middle" => "Centre the text between the top and bottom of the cell.",
        "Align Bottom" => "Line the text up at the bottom of the cell.",
        _ => return None,
    };
    Some(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screentip_names_the_button_its_shortcut_and_what_it_does() {
        assert_eq!(title("Bold (Ctrl+B)"), ("Bold".to_string(), Some("Ctrl+B".to_string())));
        assert_eq!(title("Draw Text\nBox"), ("Draw Text Box".to_string(), None));
        assert_eq!(title("Advertise-\nments"), ("Advertisements".to_string(), None));
        // A parenthesis that isn't a shortcut stays in the name.
        assert_eq!(title("Size (Letter)"), ("Size (Letter)".to_string(), None));
        assert_eq!(shortcut("Copy"), Some("Ctrl+C"));
        assert!(describe("Bold").is_some() && describe("Draw Text Box").is_some() && describe("Advertisements").is_some());
        assert_eq!(describe("Paragraph Settings").as_deref(), Some("See all the paragraph options in one window."));
        assert_eq!(describe("Something New"), None);
    }
}
