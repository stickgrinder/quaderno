// SPDX-License-Identifier: GPL-3.0-or-later

//! The entry editor view.
//!
//! It is a `GtkSourceView` with two Markdown decorations that style schemes
//! cannot express: a dimmed 2px bar at the left of every blockquote line, and
//! the paragraph margin that keeps the quote text clear of it.

use gtk::glib;
use gtk::graphene;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use sourceview5::subclass::prelude::*;

/// Extra left margin applied to blockquote paragraphs, in pixels.
const BLOCKQUOTE_INDENT: i32 = 12;
/// The name of the paragraph-margin tag.
const INDENT_TAG: &str = "quaderno-blockquote-indent";

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct EditorView;

    #[glib::object_subclass]
    impl ObjectSubclass for EditorView {
        const NAME: &'static str = "QuadernoEditorView";
        type Type = super::EditorView;
        type ParentType = sourceview5::View;
    }

    impl ObjectImpl for EditorView {}

    impl WidgetImpl for EditorView {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.parent_snapshot(snapshot);
            draw_blockquote_bars(&self.obj(), snapshot);
        }
    }

    impl TextViewImpl for EditorView {}
    impl ViewImpl for EditorView {}
}

glib::wrapper! {
    pub struct EditorView(ObjectSubclass<imp::EditorView>)
        @extends sourceview5::View, gtk::TextView, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
                    gtk::AccessibleText, gtk::Scrollable;
}

impl EditorView {
    /// A view showing `buffer` (the equivalent of `GtkSourceView::with_buffer`).
    pub fn with_buffer(buffer: &sourceview5::Buffer) -> Self {
        let view: Self = glib::Object::new();
        view.set_buffer(Some(buffer));
        view
    }

    /// Adds the blockquote paragraph margin. Call it after setting the view's
    /// margins: a tag's `left-margin` replaces the view's default margin rather
    /// than adding to it, so the tag uses the view margin plus the indent.
    pub fn install_blockquote_indent(&self) {
        let buffer = self.buffer();
        let tag = gtk::TextTag::builder()
            .name(INDENT_TAG)
            .left_margin(self.left_margin() + BLOCKQUOTE_INDENT)
            .build();
        buffer.tag_table().add(&tag);
        buffer.connect_changed(refresh_blockquote_indent);
        refresh_blockquote_indent(&buffer);
    }
}

/// Applies the margin tag to every line that starts a blockquote.
fn refresh_blockquote_indent(buffer: &gtk::TextBuffer) {
    let Some(tag) = buffer.tag_table().lookup(INDENT_TAG) else {
        return;
    };
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.remove_tag(&tag, &start, &end);

    for line in 0..buffer.line_count() {
        let Some(iter) = buffer.iter_at_line(line) else {
            continue;
        };
        let mut line_end = iter;
        line_end.forward_to_line_end();
        if is_blockquote_line(&buffer.text(&iter, &line_end, false)) {
            buffer.apply_tag(&tag, &iter, &line_end);
        }
    }
}

/// Draws a 2px bar at the left edge of each visible blockquote line.
fn draw_blockquote_bars(view: &EditorView, snapshot: &gtk::Snapshot) {
    let buffer = view.buffer();
    let Some(adjustment) = view.vadjustment() else {
        return;
    };

    let top = adjustment.value();
    let (first, _) = view.line_at_y(top as i32);
    let (last, _) = view.line_at_y((top + f64::from(view.height())).max(1.0) as i32);

    let dark = adw::StyleManager::default().is_dark();
    let color = if dark {
        gtk::gdk::RGBA::new(0.467, 0.463, 0.482, 1.0) // #77767b
    } else {
        gtk::gdk::RGBA::new(0.604, 0.600, 0.588, 1.0) // #9a9996
    };
    // The bar starts at the text's left edge, so it lines up with normal
    // paragraphs; the paragraph margin moves the quote inward.
    let x = view.left_margin() as f32;

    for line in first.line()..=last.line() {
        let Some(iter) = buffer.iter_at_line(line) else {
            continue;
        };
        let mut end = iter;
        end.forward_to_line_end();
        let text = buffer.text(&iter, &end, false);
        if !is_blockquote_line(&text) {
            continue;
        }

        let (y, height) = view.line_yrange(&iter);
        let (_, y) = view.buffer_to_window_coords(gtk::TextWindowType::Widget, 0, y);
        snapshot.append_color(
            &color,
            &graphene::Rect::new(x, y as f32, 2.0, height as f32),
        );
    }
}

/// Whether a line is a Markdown blockquote: up to three spaces, then `>`.
fn is_blockquote_line(text: &str) -> bool {
    let mut spaces = 0;
    for character in text.chars() {
        match character {
            ' ' if spaces < 3 => spaces += 1,
            '>' => return true,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_blockquote_line;

    #[test]
    fn detects_blockquote_lines_up_to_three_spaces() {
        for line in ["> quote", ">quote", " > quote", "   > quote", ">> nested"] {
            assert!(is_blockquote_line(line), "{line:?}");
        }
        for line in ["text > quote", "    > code", "\t> tab", "", ">"] {
            assert_eq!(is_blockquote_line(line), line == ">", "{line:?}");
        }
    }
}
