/// What a node is to a screen reader: the kit's widgets and the planned widget set.
/// Its number crosses the C ABI (`PxuiNode.role`, `PxuiA11yNode.role`): variants are appended, never renumbered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Role {
    /// Not in the accessibility tree (a layout-only node).
    Unknown,
    /// A top-level window.
    Window,
    /// A dialog or modal sheet.
    Dialog,
    /// A docked panel or pane.
    Panel,
    /// A group of related controls.
    Group,
    /// Static text that names or describes something.
    Label,
    /// A section heading.
    Heading,
    /// A block of running text.
    Paragraph,
    /// A link that navigates when activated.
    Link,
    /// A push button.
    Button,
    /// An on/off switch.
    Toggle,
    /// A check box (checked, unchecked or mixed).
    CheckBox,
    /// A radio button, one of a group.
    Radio,
    /// A slider that picks a value in a range.
    Slider,
    /// A number field with step buttons.
    SpinButton,
    /// A progress bar or spinner.
    Progress,
    /// A single-line text field.
    TextField,
    /// A multi-line text area.
    TextArea,
    /// A dropdown or combo box.
    ComboBox,
    /// A list of items.
    List,
    /// An item of a `List`.
    ListItem,
    /// A tree of expandable items.
    Tree,
    /// An item of a `Tree`.
    TreeItem,
    /// A table of rows and cells.
    Table,
    /// A row of a `Table`.
    Row,
    /// A cell of a `Row`.
    Cell,
    /// A table column's header.
    ColumnHeader,
    /// The row of tabs.
    TabList,
    /// One tab of a `TabList`.
    Tab,
    /// The content a `Tab` shows.
    TabPanel,
    /// A menu bar.
    MenuBar,
    /// A menu (popup list of commands).
    Menu,
    /// A command in a `Menu` or `MenuBar`.
    MenuItem,
    /// A toolbar.
    Toolbar,
    /// A status bar.
    StatusBar,
    /// A tooltip.
    Tooltip,
    /// A scrolling view.
    ScrollArea,
    /// A scroll bar.
    ScrollBar,
    /// A draggable divider between panes.
    Splitter,
    /// An image.
    Image,
    /// A custom-drawn area.
    Canvas,
    /// A separator line.
    Separator,
}

/// Accessibility states, bits of a `u32` (`Node::a11y`, `Ui::set_role`): a widget declares the ones its role has (`CHECKABLE`,
/// `CHECKED`, `EXPANDABLE`, `EXPANDED`, `EDITABLE`, `MULTILINE`, `SELECTED`, `MODAL`); the engine adds focus, press, enabled and
/// visibility. The values cross the C ABI as these bits (`PxuiNode.a11y`, `PxuiA11yNode.states`) and never change.
pub mod state {
    /// Can take keyboard focus.
    pub const FOCUSABLE: u32 = 1 << 0;
    /// Holds keyboard focus.
    pub const FOCUSED: u32 = 1 << 1;
    /// Accepts input (not disabled).
    pub const ENABLED: u32 = 1 << 2;
    /// Has a checked state (a check box, toggle, radio, checkable menu item).
    pub const CHECKABLE: u32 = 1 << 3;
    /// Is checked (on).
    pub const CHECKED: u32 = 1 << 4;
    /// Neither checked nor unchecked (a mixed check box).
    pub const MIXED: u32 = 1 << 5;
    /// Can be selected (a list or tree item, a tab).
    pub const SELECTABLE: u32 = 1 << 6;
    /// Is selected.
    pub const SELECTED: u32 = 1 << 7;
    /// Can expand to show children (a tree item, a dropdown).
    pub const EXPANDABLE: u32 = 1 << 8;
    /// Is expanded.
    pub const EXPANDED: u32 = 1 << 9;
    /// Its text can be edited.
    pub const EDITABLE: u32 = 1 << 10;
    /// Its text spans several lines.
    pub const MULTILINE: u32 = 1 << 11;
    /// Its value can be read but not changed.
    pub const READ_ONLY: u32 = 1 << 12;
    /// Is on screen.
    pub const VISIBLE: u32 = 1 << 13;
    /// Is held down.
    pub const PRESSED: u32 = 1 << 14;
    /// Blocks input to everything behind it.
    pub const MODAL: u32 = 1 << 15;
    /// The window holding keyboard focus.
    pub const ACTIVE: u32 = 1 << 16;
    /// Lays out or moves horizontally (a slider, a scroll bar, a splitter).
    pub const HORIZONTAL: u32 = 1 << 17;
    /// Lays out or moves vertically.
    pub const VERTICAL: u32 = 1 << 18;
    /// Is loading or working; its content may change.
    pub const BUSY: u32 = 1 << 19;
    /// Is the default action (the button Return presses).
    pub const DEFAULT: u32 = 1 << 20;
}
