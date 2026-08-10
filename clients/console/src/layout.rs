use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub const MIN_WIDTH: u16 = 76;
pub const MIN_HEIGHT: u16 = 22;

/// Terminal size classes used by every page. See `docs/room-rules-and-layout-plan.md`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Density {
    Compact,
    Regular,
    Wide,
}

impl Density {
    #[must_use]
    pub const fn of(area: Rect) -> Self {
        if area.width >= 128 && area.height >= 32 {
            Self::Wide
        } else if area.width >= 100 && area.height >= 28 {
            Self::Regular
        } else {
            Self::Compact
        }
    }

    #[must_use]
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }
}

/// Terminal-cell spacing scale shared by all pages.
#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    pub page_horizontal: u16,
    pub page_vertical: u16,
    pub panel_gap: u16,
    pub panel_horizontal: u16,
    pub panel_vertical: u16,
    pub header: u16,
    pub footer: u16,
    pub field_gap: u16,
}

impl Spacing {
    #[must_use]
    pub const fn for_density(density: Density) -> Self {
        match density {
            Density::Compact => Self {
                page_horizontal: 1,
                page_vertical: 0,
                panel_gap: 1,
                panel_horizontal: 1,
                panel_vertical: 0,
                header: 2,
                footer: 3,
                field_gap: 0,
            },
            Density::Regular | Density::Wide => Self {
                page_horizontal: 2,
                page_vertical: 1,
                panel_gap: 2,
                panel_horizontal: 2,
                panel_vertical: 1,
                header: 3,
                footer: 3,
                field_gap: 1,
            },
        }
    }
}

/// Splits the frame into header, content and footer bands.
#[must_use]
pub fn frame_bands(area: Rect, spacing: Spacing) -> [Rect; 3] {
    let bands = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(spacing.header),
            Constraint::Min(6),
            Constraint::Length(spacing.footer),
        ])
        .split(area);
    [bands[0], bands[1], bands[2]]
}

/// Applies the page margin to a content band.
#[must_use]
pub fn page_area(area: Rect, spacing: Spacing) -> Rect {
    inset(area, spacing.page_horizontal, spacing.page_vertical)
}

/// Shrinks a rectangle on every side, never below zero size.
#[must_use]
pub fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    let width = area.width.saturating_sub(horizontal.saturating_mul(2));
    let height = area.height.saturating_sub(vertical.saturating_mul(2));
    Rect {
        x: area.x + horizontal.min(area.width / 2),
        y: area.y + vertical.min(area.height / 2),
        width,
        height,
    }
}

/// Two columns separated by the panel gap. `secondary` is the fixed right width.
#[must_use]
pub fn columns_with_gap(area: Rect, spacing: Spacing, secondary: u16) -> [Rect; 2] {
    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(spacing.panel_gap),
            Constraint::Length(secondary),
        ])
        .split(area);
    [parts[0], parts[2]]
}

/// Two rows separated by the panel gap. `secondary` is the fixed bottom height.
#[must_use]
pub fn rows_with_gap(area: Rect, spacing: Spacing, secondary: u16) -> [Rect; 2] {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(spacing.panel_gap),
            Constraint::Length(secondary),
        ])
        .split(area);
    [parts[0], parts[2]]
}

/// Two columns weighted by ratio, separated by the panel gap.
#[must_use]
pub fn ratio_columns(area: Rect, spacing: Spacing, left: u16, right: u16) -> [Rect; 2] {
    let parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(u32::from(left), u32::from(left + right)),
            Constraint::Length(spacing.panel_gap),
            Constraint::Min(1),
        ])
        .split(area);
    [parts[0], parts[2]]
}

#[must_use]
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

/// Scroll offset in item units that keeps `active` visible with one item of context.
#[must_use]
pub const fn scroll_offset(active: usize, total: usize, visible: usize) -> usize {
    if visible == 0 || total <= visible {
        return 0;
    }
    let max_offset = total - visible;
    let lead = if active > 0 { active - 1 } else { 0 };
    if lead > max_offset { max_offset } else { lead }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::{Density, Spacing, inset, scroll_offset};

    #[test]
    fn density_follows_documented_breakpoints() {
        assert_eq!(Density::of(Rect::new(0, 0, 76, 22)), Density::Compact);
        assert_eq!(Density::of(Rect::new(0, 0, 100, 27)), Density::Compact);
        assert_eq!(Density::of(Rect::new(0, 0, 100, 30)), Density::Regular);
        assert_eq!(Density::of(Rect::new(0, 0, 144, 42)), Density::Wide);
    }

    #[test]
    fn compact_spacing_is_tighter_than_regular() {
        let compact = Spacing::for_density(Density::Compact);
        let regular = Spacing::for_density(Density::Regular);
        assert!(compact.page_horizontal < regular.page_horizontal);
        assert_eq!(compact.field_gap, 0);
        assert_eq!(regular.field_gap, 1);
        assert_eq!(compact.footer, regular.footer);
    }

    #[test]
    fn inset_never_underflows() {
        let area = inset(Rect::new(0, 0, 3, 1), 4, 4);
        assert_eq!(area.width, 0);
        assert_eq!(area.height, 0);
    }

    #[test]
    fn scroll_keeps_active_item_visible() {
        assert_eq!(scroll_offset(0, 10, 4), 0);
        assert_eq!(scroll_offset(1, 10, 4), 0);
        assert_eq!(scroll_offset(5, 10, 4), 4);
        assert_eq!(scroll_offset(9, 10, 4), 6);
        assert_eq!(scroll_offset(3, 3, 4), 0);
    }
}
