/// What a node is to a screen reader: the kit's widgets and the planned widget set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Role {
    Unknown,
    Window,
    Dialog,
    Panel,
    Group,
    Label,
    Heading,
    Paragraph,
    Link,
    Button,
    Toggle,
    CheckBox,
    Radio,
    Slider,
    SpinButton,
    Progress,
    TextField,
    TextArea,
    ComboBox,
    List,
    ListItem,
    Tree,
    TreeItem,
    Table,
    Row,
    Cell,
    ColumnHeader,
    TabList,
    Tab,
    TabPanel,
    MenuBar,
    Menu,
    MenuItem,
    Toolbar,
    StatusBar,
    Tooltip,
    ScrollArea,
    ScrollBar,
    Splitter,
    Image,
    Canvas,
    Separator,
}

/// Node states, a bitmask.
pub mod state {
    pub const FOCUSABLE: u32 = 1 << 0;
    pub const FOCUSED: u32 = 1 << 1;
    pub const ENABLED: u32 = 1 << 2;
    pub const CHECKABLE: u32 = 1 << 3;
    pub const CHECKED: u32 = 1 << 4;
    /// neither checked nor unchecked (a mixed checkbox)
    pub const MIXED: u32 = 1 << 5;
    pub const SELECTABLE: u32 = 1 << 6;
    pub const SELECTED: u32 = 1 << 7;
    pub const EXPANDABLE: u32 = 1 << 8;
    pub const EXPANDED: u32 = 1 << 9;
    pub const EDITABLE: u32 = 1 << 10;
    pub const MULTILINE: u32 = 1 << 11;
    pub const READ_ONLY: u32 = 1 << 12;
    pub const VISIBLE: u32 = 1 << 13;
    pub const PRESSED: u32 = 1 << 14;
    pub const MODAL: u32 = 1 << 15;
    /// the window holding keyboard focus
    pub const ACTIVE: u32 = 1 << 16;
    pub const HORIZONTAL: u32 = 1 << 17;
    pub const VERTICAL: u32 = 1 << 18;
    pub const BUSY: u32 = 1 << 19;
    pub const DEFAULT: u32 = 1 << 20;
}
