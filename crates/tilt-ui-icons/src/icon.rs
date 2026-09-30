//! Public icon names, lookup and SVG export.

use std::fmt::Write;

use crate::{IconSize, glyphs};

macro_rules! catalog {
    ($($variant:ident => ($name:literal, $body:expr),)+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Icon { $($variant,)+ }

        impl Icon {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            pub const fn name(self) -> &'static str {
                match self { $(Self::$variant => $name,)+ }
            }

            /// Resolves a kebab-case or snake_case catalog name.
            pub fn from_name(name: &str) -> Option<Self> {
                let name = name.trim().to_ascii_lowercase().replace('_', "-");
                match name.as_str() { $($name => Some(Self::$variant),)+ _ => None }
            }

            /// Whether the runtime animates this icon when rendered as `<icon>`.
            pub const fn is_animated(self) -> bool {
                matches!(self, Self::LoadingSpin | Self::LoadingDots | Self::HeartBeat |
                    Self::BellRing | Self::SparklePulse | Self::HourglassFlip |
                    Self::WifiPulse | Self::OrbitSpin | Self::FlameDance | Self::ArrowSlide |
                    Self::BevyFlight | Self::DiscordPulse | Self::CatBounce | Self::DogWag |
                    Self::BirdFlap | Self::RabbitHop | Self::PawStep | Self::GamepadShake |
                    Self::DiceRoll | Self::SwordSwing | Self::BookPulse | Self::MusicBounce |
                    Self::CameraFlash | Self::CloudDrift | Self::SunSpin | Self::MoonRock |
                    Self::RocketLaunch | Self::MessagePop | Self::CheckBounce | Self::DownloadDrop)
            }

            pub const fn body(self) -> &'static str {
                match self { $(Self::$variant => $body,)+ }
            }

            /// An embeddable TiltUI image/CSS source, e.g. `tilt-icon:home@32`.
            pub fn source(self, size: IconSize) -> String {
                format!("tilt-icon:{}@{}", self.name(), size.pixels())
            }

            /// Standalone SVG using the CSS `currentColor` of its host.
            pub fn svg(self, size: IconSize) -> String {
                let pixels = size.pixels();
                let mut result = String::with_capacity(self.body().len() + 210);
                write!(result, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{pixels}\" height=\"{pixels}\" viewBox=\"0 0 24 24\"><g fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.8\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</g></svg>", self.body()).expect("writing to String cannot fail");
                result
            }
        }
    };
}

catalog! {
    // Homes: five distinct architectural treatments.
    Home => ("home", glyphs::HOME),
    HomeFilled => ("home-filled", glyphs::HOME_FILLED),
    HomeModern => ("home-modern", glyphs::HOME_MODERN),
    HomeRoof => ("home-roof", glyphs::HOME_ROOF),
    HomeCircle => ("home-circle", glyphs::HOME_CIRCLE),

    // Eight settings variants, including gears and adjustment controls.
    Settings => ("settings", glyphs::SETTINGS),
    SettingsFilled => ("settings-filled", glyphs::SETTINGS_FILLED),
    SettingsSliders => ("settings-sliders", glyphs::SETTINGS_SLIDERS),
    SettingsTune => ("settings-tune", glyphs::SETTINGS_TUNE),
    SettingsWrench => ("settings-wrench", glyphs::SETTINGS_WRENCH),
    SettingsAdjust => ("settings-adjust", glyphs::SETTINGS_ADJUST),
    SettingsCircle => ("settings-circle", glyphs::SETTINGS_CIRCLE),
    SettingsHorizontal => ("settings-horizontal", glyphs::SETTINGS_HORIZONTAL),

    // Six person variants.
    User => ("user", glyphs::USER),
    UserFilled => ("user-filled", glyphs::USER_FILLED),
    UserCircle => ("user-circle", glyphs::USER_CIRCLE),
    UserSquare => ("user-square", glyphs::USER_SQUARE),
    UserPlus => ("user-plus", glyphs::USER_PLUS),
    UserGroup => ("user-group", glyphs::USER_GROUP),

    // Ten menu treatments.
    Menu => ("menu", glyphs::MENU),
    MenuWide => ("menu-wide", glyphs::MENU_WIDE),
    MenuCompact => ("menu-compact", glyphs::MENU_COMPACT),
    MenuDots => ("menu-dots", glyphs::MENU_DOTS),
    MenuDotsVertical => ("menu-dots-vertical", glyphs::MENU_DOTS_VERTICAL),
    MenuGrid => ("menu-grid", glyphs::MENU_GRID),
    MenuGridFilled => ("menu-grid-filled", glyphs::MENU_GRID_FILLED),
    MenuList => ("menu-list", glyphs::MENU_LIST),
    MenuChevron => ("menu-chevron", glyphs::MENU_CHEVRON),
    MenuCircle => ("menu-circle", glyphs::MENU_CIRCLE),

    Close => ("close", glyphs::CLOSE),
    CloseCircle => ("close-circle", glyphs::CLOSE_CIRCLE),
    CloseSquare => ("close-square", glyphs::CLOSE_SQUARE),
    CloseBold => ("close-bold", glyphs::CLOSE_BOLD),

    Briefcase => ("briefcase", glyphs::BRIEFCASE),
    BriefcaseFilled => ("briefcase-filled", glyphs::BRIEFCASE_FILLED),
    BriefcaseMedical => ("briefcase-medical", glyphs::BRIEFCASE_MEDICAL),
    BriefcaseBusiness => ("briefcase-business", glyphs::BRIEFCASE_BUSINESS),

    // Common application icons beyond the requested families.
    Search => ("search", glyphs::SEARCH),
    Bell => ("bell", glyphs::BELL),
    Heart => ("heart", glyphs::HEART),
    Star => ("star", glyphs::STAR),
    Check => ("check", glyphs::CHECK),
    Plus => ("plus", glyphs::PLUS),
    Minus => ("minus", glyphs::MINUS),
    ArrowLeft => ("arrow-left", glyphs::ARROW_LEFT),
    ArrowRight => ("arrow-right", glyphs::ARROW_RIGHT),
    ChevronLeft => ("chevron-left", glyphs::CHEVRON_LEFT),
    ChevronRight => ("chevron-right", glyphs::CHEVRON_RIGHT),
    ChevronDown => ("chevron-down", glyphs::CHEVRON_DOWN),
    Download => ("download", glyphs::DOWNLOAD),
    Upload => ("upload", glyphs::UPLOAD),
    Trash => ("trash", glyphs::TRASH),
    Edit => ("edit", glyphs::EDIT),
    Mail => ("mail", glyphs::MAIL),
    Calendar => ("calendar", glyphs::CALENDAR),
    Clock => ("clock", glyphs::CLOCK),
    Folder => ("folder", glyphs::FOLDER),
    File => ("file", glyphs::FILE),
    Lock => ("lock", glyphs::LOCK),
    Eye => ("eye", glyphs::EYE),
    Sun => ("sun", glyphs::SUN),
    Moon => ("moon", glyphs::MOON),

    Copy => ("copy", glyphs::COPY),
    Link => ("link", glyphs::LINK),
    ExternalLink => ("external-link", glyphs::EXTERNAL_LINK),
    Share => ("share", glyphs::SHARE),
    Filter => ("filter", glyphs::FILTER),
    Refresh => ("refresh", glyphs::REFRESH),
    Bookmark => ("bookmark", glyphs::BOOKMARK),
    MapPin => ("map-pin", glyphs::MAP_PIN),
    Info => ("info", glyphs::INFO),
    Warning => ("warning", glyphs::WARNING),
    HelpCircle => ("help-circle", glyphs::HELP_CIRCLE),
    CheckCircle => ("check-circle", glyphs::CHECK_CIRCLE),
    Play => ("play", glyphs::PLAY),
    Pause => ("pause", glyphs::PAUSE),
    Image => ("image", glyphs::IMAGE),
    Phone => ("phone", glyphs::PHONE),

    // Navigation and page structure.
    Dashboard => ("dashboard", glyphs::DASHBOARD),
    List => ("list", glyphs::LIST),
    Grid => ("grid", glyphs::GRID),
    Layers => ("layers", glyphs::LAYERS),
    Target => ("target", glyphs::TARGET),
    Globe => ("globe", glyphs::GLOBE),
    Compass => ("compass", glyphs::COMPASS),
    Map => ("map", glyphs::MAP),
    Route => ("route", glyphs::ROUTE),
    ArrowUp => ("arrow-up", glyphs::ARROW_UP),
    ArrowDown => ("arrow-down", glyphs::ARROW_DOWN),
    ChevronUp => ("chevron-up", glyphs::CHEVRON_UP),

    // Devices, media, and commerce.
    Camera => ("camera", glyphs::CAMERA),
    Video => ("video", glyphs::VIDEO),
    Music => ("music", glyphs::MUSIC),
    Mic => ("mic", glyphs::MIC),
    Volume => ("volume", glyphs::VOLUME),
    Wifi => ("wifi", glyphs::WIFI),
    Battery => ("battery", glyphs::BATTERY),
    Monitor => ("monitor", glyphs::MONITOR),
    Smartphone => ("smartphone", glyphs::SMARTPHONE),
    Printer => ("printer", glyphs::PRINTER),
    Gift => ("gift", glyphs::GIFT),
    ShoppingCart => ("shopping-cart", glyphs::SHOPPING_CART),

    // Animated variants gain motion when used as <icon> elements.
    LoadingSpin => ("loading-spin", glyphs::REFRESH),
    LoadingDots => ("loading-dots", glyphs::ELLIPSIS),
    HeartBeat => ("heart-beat", glyphs::HEART),
    BellRing => ("bell-ring", glyphs::BELL),
    SparklePulse => ("sparkle-pulse", glyphs::STAR),
    HourglassFlip => ("hourglass-flip", glyphs::HOURGLASS),
    WifiPulse => ("wifi-pulse", glyphs::WIFI),
    OrbitSpin => ("orbit-spin", glyphs::SATELLITE),
    FlameDance => ("flame-dance", glyphs::FLAME),
    ArrowSlide => ("arrow-slide", glyphs::ARROW_RIGHT),

    // Five reusable Bevy bird SVG variants.
    Bevy => ("bevy", glyphs::BEVY),
    BevyCircle => ("bevy-circle", glyphs::BEVY_CIRCLE),
    BevyBadge => ("bevy-badge", glyphs::BEVY_BADGE),
    BevyOrbit => ("bevy-orbit", glyphs::BEVY_ORBIT),
    BevySparkle => ("bevy-sparkle", glyphs::BEVY_SPARKLE),

    // Productivity: Lucide SVG geometry.
    AlarmClock => ("alarm-clock", glyphs::ALARM_CLOCK),
    CalendarDays => ("calendar-days", glyphs::CALENDAR_DAYS),
    CalendarCheck => ("calendar-check", glyphs::CALENDAR_CHECK),
    Timer => ("timer", glyphs::TIMER),
    ListChecks => ("list-checks", glyphs::LIST_CHECKS),
    Notebook => ("notebook", glyphs::NOTEBOOK),
    Clipboard => ("clipboard", glyphs::CLIPBOARD),
    ClipboardCheck => ("clipboard-check", glyphs::CLIPBOARD_CHECK),
    StickyNote => ("sticky-note", glyphs::STICKY_NOTE),
    BookOpen => ("book-open", glyphs::BOOK_OPEN),

    // Development: Lucide SVG geometry.
    Bug => ("bug", glyphs::BUG),
    GitBranch => ("git-branch", glyphs::GIT_BRANCH),
    GitCommitHorizontal => ("git-commit-horizontal", glyphs::GIT_COMMIT_HORIZONTAL),
    GitMerge => ("git-merge", glyphs::GIT_MERGE),
    Braces => ("braces", glyphs::BRACES),
    Brackets => ("brackets", glyphs::BRACKETS),
    Binary => ("binary", glyphs::BINARY),
    Cpu => ("cpu", glyphs::CPU),
    HardDrive => ("hard-drive", glyphs::HARD_DRIVE),
    Keyboard => ("keyboard", glyphs::KEYBOARD),

    // Health: Lucide SVG geometry.
    Ambulance => ("ambulance", glyphs::AMBULANCE),
    Bandage => ("bandage", glyphs::BANDAGE),
    HeartPulse => ("heart-pulse", glyphs::HEART_PULSE),
    Pill => ("pill", glyphs::PILL),
    Syringe => ("syringe", glyphs::SYRINGE),
    Stethoscope => ("stethoscope", glyphs::STETHOSCOPE),
    Hospital => ("hospital", glyphs::HOSPITAL),
    Dna => ("dna", glyphs::DNA),
    Microscope => ("microscope", glyphs::MICROSCOPE),
    FlaskConical => ("flask-conical", glyphs::FLASK_CONICAL),

    // Food & drink: Lucide SVG geometry.
    Apple => ("apple", glyphs::APPLE),
    Banana => ("banana", glyphs::BANANA),
    Cake => ("cake", glyphs::CAKE),
    ChefHat => ("chef-hat", glyphs::CHEF_HAT),
    Cookie => ("cookie", glyphs::COOKIE),
    CupSoda => ("cup-soda", glyphs::CUP_SODA),
    Fish => ("fish", glyphs::FISH),
    Pizza => ("pizza", glyphs::PIZZA),
    Sandwich => ("sandwich", glyphs::SANDWICH),
    Wine => ("wine", glyphs::WINE),

    // Sports & outdoors: Lucide SVG geometry.
    Dumbbell => ("dumbbell", glyphs::DUMBBELL),
    Award => ("award", glyphs::AWARD),
    Goal => ("goal", glyphs::GOAL),
    Medal => ("medal", glyphs::MEDAL),
    PersonStanding => ("person-standing", glyphs::PERSON_STANDING),
    Trophy => ("trophy", glyphs::TROPHY),
    Volleyball => ("volleyball", glyphs::VOLLEYBALL),
    WavesLadder => ("waves-ladder", glyphs::WAVES_LADDER),
    Sailboat => ("sailboat", glyphs::SAILBOAT),
    Tent => ("tent", glyphs::TENT),

    // Social: Lucide SVG geometry.
    UsersRound => ("users-round", glyphs::USERS_ROUND),
    UserRoundPlus => ("user-round-plus", glyphs::USER_ROUND_PLUS),
    UserRoundSearch => ("user-round-search", glyphs::USER_ROUND_SEARCH),
    MessageSquareHeart => ("message-square-heart", glyphs::MESSAGE_SQUARE_HEART),
    HeartPlus => ("heart-plus", glyphs::HEART_PLUS),
    ThumbsUp => ("thumbs-up", glyphs::THUMBS_UP),
    ThumbsDown => ("thumbs-down", glyphs::THUMBS_DOWN),
    Handshake => ("handshake", glyphs::HANDSHAKE),
    PartyPopper => ("party-popper", glyphs::PARTY_POPPER),
    HeartHandshake => ("heart-handshake", glyphs::HEART_HANDSHAKE),

    // App layout: Lucide SVG geometry.
    PanelLeft => ("panel-left", glyphs::PANEL_LEFT),
    PanelRight => ("panel-right", glyphs::PANEL_RIGHT),
    PanelLeftOpen => ("panel-left-open", glyphs::PANEL_LEFT_OPEN),
    PanelLeftClose => ("panel-left-close", glyphs::PANEL_LEFT_CLOSE),
    AppWindow => ("app-window", glyphs::APP_WINDOW),
    Columns2 => ("columns-2", glyphs::COLUMNS_2),
    Rows2 => ("rows-2", glyphs::ROWS_2),
    LayoutTemplate => ("layout-template", glyphs::LAYOUT_TEMPLATE),
    Split => ("split", glyphs::SPLIT),
    BetweenHorizontalStart => ("between-horizontal-start", glyphs::BETWEEN_HORIZONTAL_START),

    // Finance: Lucide SVG geometry.
    BadgeDollarSign => ("badge-dollar-sign", glyphs::BADGE_DOLLAR_SIGN),
    CircleDollarSign => ("circle-dollar-sign", glyphs::CIRCLE_DOLLAR_SIGN),
    Landmark => ("landmark", glyphs::LANDMARK),
    PiggyBank => ("piggy-bank", glyphs::PIGGY_BANK),
    HandCoins => ("hand-coins", glyphs::HAND_COINS),
    ChartNoAxesCombined => ("chart-no-axes-combined", glyphs::CHART_NO_AXES_COMBINED),
    Calculator => ("calculator", glyphs::CALCULATOR),
    ScanLine => ("scan-line", glyphs::SCAN_LINE),
    QrCode => ("qr-code", glyphs::QR_CODE),
    Ticket => ("ticket", glyphs::TICKET),

    // Science & tech: Lucide SVG geometry.
    Atom => ("atom", glyphs::ATOM),
    Beaker => ("beaker", glyphs::BEAKER),
    FlaskRound => ("flask-round", glyphs::FLASK_ROUND),
    Telescope => ("telescope", glyphs::TELESCOPE),
    Satellite => ("satellite", glyphs::SATELLITE),
    Bot => ("bot", glyphs::BOT),
    CircuitBoard => ("circuit-board", glyphs::CIRCUIT_BOARD),
    Microchip => ("microchip", glyphs::MICROCHIP),
    Zap => ("zap", glyphs::ZAP),
    RadioTower => ("radio-tower", glyphs::RADIO_TOWER),

    // Everyday objects: Lucide SVG geometry.
    Anchor => ("anchor", glyphs::ANCHOR),
    Crown => ("crown", glyphs::CROWN),
    Diamond => ("diamond", glyphs::DIAMOND),
    Gem => ("gem", glyphs::GEM),
    Lightbulb => ("lightbulb", glyphs::LIGHTBULB),
    Puzzle => ("puzzle", glyphs::PUZZLE),
    Rocket => ("rocket", glyphs::ROCKET),
    Snowflake => ("snowflake", glyphs::SNOWFLAKE),
    TreePine => ("tree-pine", glyphs::TREE_PINE),
    WandSparkles => ("wand-sparkles", glyphs::WAND_SPARKLES),

    // Actions: Lucide SVG geometry.
    CheckCheck => ("check-check", glyphs::CHECK_CHECK),
    CircleX => ("circle-x", glyphs::CIRCLE_X),
    Ellipsis => ("ellipsis", glyphs::ELLIPSIS),
    EllipsisVertical => ("ellipsis-vertical", glyphs::ELLIPSIS_VERTICAL),
    RotateCcw => ("rotate-ccw", glyphs::ROTATE_CCW),
    RotateCw => ("rotate-cw", glyphs::ROTATE_CW),
    Undo2 => ("undo-2", glyphs::UNDO_2),
    Redo2 => ("redo-2", glyphs::REDO_2),
    Maximize => ("maximize", glyphs::MAXIMIZE),
    Minimize => ("minimize", glyphs::MINIMIZE),

    // Files & cloud: Lucide SVG geometry.
    FilePlus => ("file-plus", glyphs::FILE_PLUS),
    FileMinus => ("file-minus", glyphs::FILE_MINUS),
    FileCheck => ("file-check", glyphs::FILE_CHECK),
    FileSearch => ("file-search", glyphs::FILE_SEARCH),
    FolderOpen => ("folder-open", glyphs::FOLDER_OPEN),
    FolderPlus => ("folder-plus", glyphs::FOLDER_PLUS),
    Archive => ("archive", glyphs::ARCHIVE),
    Cloud => ("cloud", glyphs::CLOUD),
    CloudUpload => ("cloud-upload", glyphs::CLOUD_UPLOAD),
    CloudDownload => ("cloud-download", glyphs::CLOUD_DOWNLOAD),

    // Commerce: Lucide SVG geometry.
    ShoppingBag => ("shopping-bag", glyphs::SHOPPING_BAG),
    CreditCard => ("credit-card", glyphs::CREDIT_CARD),
    Wallet => ("wallet", glyphs::WALLET),
    Receipt => ("receipt", glyphs::RECEIPT),
    Tag => ("tag", glyphs::TAG),
    Tags => ("tags", glyphs::TAGS),
    Percent => ("percent", glyphs::PERCENT),
    Coins => ("coins", glyphs::COINS),
    Banknote => ("banknote", glyphs::BANKNOTE),
    Store => ("store", glyphs::STORE),

    // Communication: Lucide SVG geometry.
    MessageSquare => ("message-square", glyphs::MESSAGE_SQUARE),
    MessageCircle => ("message-circle", glyphs::MESSAGE_CIRCLE),
    Send => ("send", glyphs::SEND),
    Inbox => ("inbox", glyphs::INBOX),
    AtSign => ("at-sign", glyphs::AT_SIGN),
    Hash => ("hash", glyphs::HASH),
    Headphones => ("headphones", glyphs::HEADPHONES),
    Megaphone => ("megaphone", glyphs::MEGAPHONE),
    Radio => ("radio", glyphs::RADIO),
    Contact => ("contact", glyphs::CONTACT),

    // Media: Lucide SVG geometry.
    SkipBack => ("skip-back", glyphs::SKIP_BACK),
    SkipForward => ("skip-forward", glyphs::SKIP_FORWARD),
    Rewind => ("rewind", glyphs::REWIND),
    FastForward => ("fast-forward", glyphs::FAST_FORWARD),
    Square => ("square", glyphs::SQUARE),
    CircleDot => ("circle-dot", glyphs::CIRCLE_DOT),
    VolumeX => ("volume-x", glyphs::VOLUME_X),
    Film => ("film", glyphs::FILM),
    Scan => ("scan", glyphs::SCAN),
    PictureInPicture => ("picture-in-picture", glyphs::PICTURE_IN_PICTURE),

    // Editor: Lucide SVG geometry.
    Crop => ("crop", glyphs::CROP),
    Scissors => ("scissors", glyphs::SCISSORS),
    Brush => ("brush", glyphs::BRUSH),
    PaintBucket => ("paint-bucket", glyphs::PAINT_BUCKET),
    PenTool => ("pen-tool", glyphs::PEN_TOOL),
    Eraser => ("eraser", glyphs::ERASER),
    Ruler => ("ruler", glyphs::RULER),
    Move => ("move", glyphs::MOVE),
    ListIndentIncrease => ("list-indent-increase", glyphs::LIST_INDENT_INCREASE),
    ListIndentDecrease => ("list-indent-decrease", glyphs::LIST_INDENT_DECREASE),

    // Weather & nature: Lucide SVG geometry.
    CloudSun => ("cloud-sun", glyphs::CLOUD_SUN),
    CloudRain => ("cloud-rain", glyphs::CLOUD_RAIN),
    CloudSnow => ("cloud-snow", glyphs::CLOUD_SNOW),
    Wind => ("wind", glyphs::WIND),
    Umbrella => ("umbrella", glyphs::UMBRELLA),
    Thermometer => ("thermometer", glyphs::THERMOMETER),
    Droplet => ("droplet", glyphs::DROPLET),
    Flame => ("flame", glyphs::FLAME),
    Leaf => ("leaf", glyphs::LEAF),
    Mountain => ("mountain", glyphs::MOUNTAIN),

    // Data & code: Lucide SVG geometry.
    ChartColumn => ("chart-column", glyphs::CHART_COLUMN),
    ChartLine => ("chart-line", glyphs::CHART_LINE),
    ChartPie => ("chart-pie", glyphs::CHART_PIE),
    TrendingUp => ("trending-up", glyphs::TRENDING_UP),
    TrendingDown => ("trending-down", glyphs::TRENDING_DOWN),
    Activity => ("activity", glyphs::ACTIVITY),
    Database => ("database", glyphs::DATABASE),
    Server => ("server", glyphs::SERVER),
    Terminal => ("terminal", glyphs::TERMINAL),
    Code => ("code", glyphs::CODE),

    // Security: Lucide SVG geometry.
    Shield => ("shield", glyphs::SHIELD),
    ShieldCheck => ("shield-check", glyphs::SHIELD_CHECK),
    Key => ("key", glyphs::KEY),
    ScanFace => ("scan-face", glyphs::SCAN_FACE),
    LockOpen => ("lock-open", glyphs::LOCK_OPEN),
    UserRoundCheck => ("user-round-check", glyphs::USER_ROUND_CHECK),
    UserRoundMinus => ("user-round-minus", glyphs::USER_ROUND_MINUS),
    LogIn => ("log-in", glyphs::LOG_IN),
    LogOut => ("log-out", glyphs::LOG_OUT),
    ContactRound => ("contact-round", glyphs::CONTACT_ROUND),

    // Travel & places: Lucide SVG geometry.
    Car => ("car", glyphs::CAR),
    Bus => ("bus", glyphs::BUS),
    TrainFront => ("train-front", glyphs::TRAIN_FRONT),
    Plane => ("plane", glyphs::PLANE),
    Bike => ("bike", glyphs::BIKE),
    Ship => ("ship", glyphs::SHIP),
    Building => ("building", glyphs::BUILDING),
    Coffee => ("coffee", glyphs::COFFEE),
    Utensils => ("utensils", glyphs::UTENSILS),
    Flag => ("flag", glyphs::FLAG),
    // Animals, games, learning, home, creative tools and mobility: Lucide.
    // Animals.
    Cat => ("cat", glyphs::CAT),
    Dog => ("dog", glyphs::DOG),
    Rabbit => ("rabbit", glyphs::RABBIT),
    Bird => ("bird", glyphs::BIRD),
    Birdhouse => ("birdhouse", glyphs::BIRDHOUSE),
    Turtle => ("turtle", glyphs::TURTLE),
    Squirrel => ("squirrel", glyphs::SQUIRREL),
    PawPrint => ("paw-print", glyphs::PAW_PRINT),
    Snail => ("snail", glyphs::SNAIL),
    Worm => ("worm", glyphs::WORM),
    // Wildlife.
    Rat => ("rat", glyphs::RAT),
    FishOff => ("fish-off", glyphs::FISH_OFF),
    FishSymbol => ("fish-symbol", glyphs::FISH_SYMBOL),
    FishingHook => ("fishing-hook", glyphs::FISHING_HOOK),
    FishingRod => ("fishing-rod", glyphs::FISHING_ROD),
    BugOff => ("bug-off", glyphs::BUG_OFF),
    BugPlay => ("bug-play", glyphs::BUG_PLAY),
    Egg => ("egg", glyphs::EGG),
    Shell => ("shell", glyphs::SHELL),
    Feather => ("feather", glyphs::FEATHER),
    // Gaming.
    Gamepad => ("gamepad", glyphs::GAMEPAD),
    Gamepad2 => ("gamepad-2", glyphs::GAMEPAD_2),
    GamepadDirectional => ("gamepad-directional", glyphs::GAMEPAD_DIRECTIONAL),
    Dice1 => ("dice-1", glyphs::DICE_1),
    Dice2 => ("dice-2", glyphs::DICE_2),
    Dice3 => ("dice-3", glyphs::DICE_3),
    Dice4 => ("dice-4", glyphs::DICE_4),
    Dice5 => ("dice-5", glyphs::DICE_5),
    Dice6 => ("dice-6", glyphs::DICE_6),
    Dices => ("dices", glyphs::DICES),
    // Boardgames.
    ChessBishop => ("chess-bishop", glyphs::CHESS_BISHOP),
    ChessKing => ("chess-king", glyphs::CHESS_KING),
    ChessKnight => ("chess-knight", glyphs::CHESS_KNIGHT),
    ChessPawn => ("chess-pawn", glyphs::CHESS_PAWN),
    ChessQueen => ("chess-queen", glyphs::CHESS_QUEEN),
    ChessRook => ("chess-rook", glyphs::CHESS_ROOK),
    Joystick => ("joystick", glyphs::JOYSTICK),
    Sword => ("sword", glyphs::SWORD),
    Swords => ("swords", glyphs::SWORDS),
    BowArrow => ("bow-arrow", glyphs::BOW_ARROW),
    // Learning.
    GraduationCap => ("graduation-cap", glyphs::GRADUATION_CAP),
    School => ("school", glyphs::SCHOOL),
    Book => ("book", glyphs::BOOK),
    BookText => ("book-text", glyphs::BOOK_TEXT),
    BookCheck => ("book-check", glyphs::BOOK_CHECK),
    Library => ("library", glyphs::LIBRARY),
    Pencil => ("pencil", glyphs::PENCIL),
    PencilLine => ("pencil-line", glyphs::PENCIL_LINE),
    Backpack => ("backpack", glyphs::BACKPACK),
    ScrollText => ("scroll-text", glyphs::SCROLL_TEXT),
    // Household.
    Sofa => ("sofa", glyphs::SOFA),
    Bed => ("bed", glyphs::BED),
    BedDouble => ("bed-double", glyphs::BED_DOUBLE),
    Bath => ("bath", glyphs::BATH),
    ShowerHead => ("shower-head", glyphs::SHOWER_HEAD),
    Lamp => ("lamp", glyphs::LAMP),
    LampDesk => ("lamp-desk", glyphs::LAMP_DESK),
    DoorOpen => ("door-open", glyphs::DOOR_OPEN),
    DoorClosed => ("door-closed", glyphs::DOOR_CLOSED),
    HousePlus => ("house-plus", glyphs::HOUSE_PLUS),
    // Creative.
    Palette => ("palette", glyphs::PALETTE),
    Paintbrush => ("paintbrush", glyphs::PAINTBRUSH),
    PaintRoller => ("paint-roller", glyphs::PAINT_ROLLER),
    Aperture => ("aperture", glyphs::APERTURE),
    CameraOff => ("camera-off", glyphs::CAMERA_OFF),
    Clapperboard => ("clapperboard", glyphs::CLAPPERBOARD),
    Drum => ("drum", glyphs::DRUM),
    Guitar => ("guitar", glyphs::GUITAR),
    Piano => ("piano", glyphs::PIANO),
    Speaker => ("speaker", glyphs::SPEAKER),
    // Mobility.
    TrainTrack => ("train-track", glyphs::TRAIN_TRACK),
    TramFront => ("tram-front", glyphs::TRAM_FRONT),
    Truck => ("truck", glyphs::TRUCK),
    ShipWheel => ("ship-wheel", glyphs::SHIP_WHEEL),
    TrafficCone => ("traffic-cone", glyphs::TRAFFIC_CONE),
    Fuel => ("fuel", glyphs::FUEL),

    // App and tool marks: Simple Icons.
    // Apps Gaming.
    Discord => ("discord", glyphs::DISCORD),
    Steam => ("steam", glyphs::STEAM),
    EpicGames => ("epic-games", glyphs::EPIC_GAMES),
    ItchIo => ("itch-io", glyphs::ITCH_IO),
    Gog => ("gog", glyphs::GOG),
    Godot => ("godot", glyphs::GODOT),
    Unity => ("unity", glyphs::UNITY),
    UnrealEngine => ("unreal-engine", glyphs::UNREAL_ENGINE),
    // Apps Social.
    Youtube => ("youtube", glyphs::YOUTUBE),
    Twitch => ("twitch", glyphs::TWITCH),
    Reddit => ("reddit", glyphs::REDDIT),
    Spotify => ("spotify", glyphs::SPOTIFY),
    Telegram => ("telegram", glyphs::TELEGRAM),
    Whatsapp => ("whatsapp", glyphs::WHATSAPP),
    Instagram => ("instagram", glyphs::INSTAGRAM),
    Tiktok => ("tiktok", glyphs::TIKTOK),
    // Apps Tools.
    Github => ("github", glyphs::GITHUB),
    Gitlab => ("gitlab", glyphs::GITLAB),
    Figma => ("figma", glyphs::FIGMA),
    Blender => ("blender", glyphs::BLENDER),
    RustLogo => ("rust-logo", glyphs::RUST_LOGO),
    Docker => ("docker", glyphs::DOCKER),
    ObsStudio => ("obs-studio", glyphs::OBS_STUDIO),
    Notion => ("notion", glyphs::NOTION),

    // Additional animated variants: lightweight transforms on shared SVG textures.
    BevyFlight => ("bevy-flight", glyphs::BEVY_SPARKLE),
    DiscordPulse => ("discord-pulse", glyphs::DISCORD),
    CatBounce => ("cat-bounce", glyphs::CAT),
    DogWag => ("dog-wag", glyphs::DOG),
    BirdFlap => ("bird-flap", glyphs::BIRD),
    RabbitHop => ("rabbit-hop", glyphs::RABBIT),
    PawStep => ("paw-step", glyphs::PAW_PRINT),
    GamepadShake => ("gamepad-shake", glyphs::GAMEPAD),
    DiceRoll => ("dice-roll", glyphs::DICES),
    SwordSwing => ("sword-swing", glyphs::SWORD),
    BookPulse => ("book-pulse", glyphs::BOOK),
    MusicBounce => ("music-bounce", glyphs::MUSIC),
    CameraFlash => ("camera-flash", glyphs::CAMERA),
    CloudDrift => ("cloud-drift", glyphs::CLOUD),
    SunSpin => ("sun-spin", glyphs::SUN),
    MoonRock => ("moon-rock", glyphs::MOON),
    RocketLaunch => ("rocket-launch", glyphs::ROCKET),
    MessagePop => ("message-pop", glyphs::MESSAGE_SQUARE),
    CheckBounce => ("check-bounce", glyphs::CHECK),
    DownloadDrop => ("download-drop", glyphs::DOWNLOAD),

    // Additional nature, animal, app and utility icons.
    // Flowers & plants.
    Flower => ("flower", glyphs::FLOWER),
    LeafMaple => ("leaf-maple", glyphs::LEAF_MAPLE),
    LeafTwo => ("leaf-two", glyphs::LEAF_TWO),
    Plant => ("plant", glyphs::PLANT),
    PlantTwo => ("plant-two", glyphs::PLANT_TWO),
    Seedling => ("seedling", glyphs::SEEDLING),
    Tree => ("tree", glyphs::TREE),
    Trees => ("trees", glyphs::TREES),
    Cactus => ("cactus", glyphs::CACTUS),
    Mushroom => ("mushroom", glyphs::MUSHROOM),

    // Sky & weather.
    Sunrise => ("sunrise", glyphs::SUNRISE),
    Sunset => ("sunset", glyphs::SUNSET),
    MoonStar => ("moon-star", glyphs::MOON_STAR),
    SunDim => ("sun-dim", glyphs::SUN_DIM),
    CloudDrizzle => ("cloud-drizzle", glyphs::CLOUD_DRIZZLE),
    CloudFog => ("cloud-fog", glyphs::CLOUD_FOG),
    CloudHail => ("cloud-hail", glyphs::CLOUD_HAIL),
    CloudLightning => ("cloud-lightning", glyphs::CLOUD_LIGHTNING),
    CloudMoon => ("cloud-moon", glyphs::CLOUD_MOON),
    CloudRainWind => ("cloud-rain-wind", glyphs::CLOUD_RAIN_WIND),

    // Land & water.
    Rainbow => ("rainbow", glyphs::RAINBOW),
    Volcano => ("volcano", glyphs::VOLCANO),
    Beach => ("beach", glyphs::BEACH),
    Windmill => ("windmill", glyphs::WINDMILL),
    Droplets => ("droplets", glyphs::DROPLETS),
    MountainSnow => ("mountain-snow", glyphs::MOUNTAIN_SNOW),
    Earth => ("earth", glyphs::EARTH),
    WavesHorizontal => ("waves-horizontal", glyphs::WAVES_HORIZONTAL),
    SunSnow => ("sun-snow", glyphs::SUN_SNOW),
    CloudSunRain => ("cloud-sun-rain", glyphs::CLOUD_SUN_RAIN),

    // More animals.
    Bat => ("bat", glyphs::BAT),
    Butterfly => ("butterfly", glyphs::BUTTERFLY),
    Deer => ("deer", glyphs::DEER),
    Horse => ("horse", glyphs::HORSE),
    Pig => ("pig", glyphs::PIG),
    Spider => ("spider", glyphs::SPIDER),
    FishBone => ("fish-bone", glyphs::FISH_BONE),
    DogBowl => ("dog-bowl", glyphs::DOG_BOWL),
    Paw => ("paw", glyphs::PAW),
    PawOff => ("paw-off", glyphs::PAW_OFF),

    // More wildlife.
    Cow => ("cow", glyphs::COW),
    BugBeetle => ("bug-beetle", glyphs::BUG_BEETLE),
    FishSimple => ("fish-simple", glyphs::FISH_SIMPLE),
    Bee => ("bee", glyphs::BEE),
    Elephant => ("elephant", glyphs::ELEPHANT),
    Kangaroo => ("kangaroo", glyphs::KANGAROO),
    Owl => ("owl", glyphs::OWL),
    Penguin => ("penguin", glyphs::PENGUIN),
    Shark => ("shark", glyphs::SHARK),
    Snake => ("snake", glyphs::SNAKE),

    // Browsers & platforms.
    Firefox => ("firefox", glyphs::FIREFOX),
    Chrome => ("chrome", glyphs::CHROME),
    Safari => ("safari", glyphs::SAFARI),
    Edge => ("edge", glyphs::EDGE),
    Opera => ("opera", glyphs::OPERA),
    Vivaldi => ("vivaldi", glyphs::VIVALDI),
    ArcBrowser => ("arc-browser", glyphs::ARC_BROWSER),
    Yandex => ("yandex", glyphs::YANDEX),
    Google => ("google", glyphs::GOOGLE),
    AppleLogo => ("apple-logo", glyphs::APPLE_LOGO),

    // Developer apps.
    Angular => ("angular", glyphs::ANGULAR),
    React => ("react", glyphs::REACT),
    Vue => ("vue", glyphs::VUE),
    Svelte => ("svelte", glyphs::SVELTE),
    Typescript => ("typescript", glyphs::TYPESCRIPT),
    Javascript => ("javascript", glyphs::JAVASCRIPT),
    Python => ("python", glyphs::PYTHON),
    VsCode => ("vs-code", glyphs::VS_CODE),
    NodeJs => ("node-js", glyphs::NODE_JS),
    Npm => ("npm", glyphs::NPM),

    // Community apps.
    Slack => ("slack", glyphs::SLACK),
    Zoom => ("zoom", glyphs::ZOOM),
    Linkedin => ("linkedin", glyphs::LINKEDIN),
    Pinterest => ("pinterest", glyphs::PINTEREST),
    Mastodon => ("mastodon", glyphs::MASTODON),
    Signal => ("signal", glyphs::SIGNAL),
    Matrix => ("matrix", glyphs::MATRIX),
    Dropbox => ("dropbox", glyphs::DROPBOX),
    Trello => ("trello", glyphs::TRELLO),
    Asana => ("asana", glyphs::ASANA),

    // Workflow utilities.
    ClipboardCopy => ("clipboard-copy", glyphs::CLIPBOARD_COPY),
    ClipboardList => ("clipboard-list", glyphs::CLIPBOARD_LIST),
    CopyCheck => ("copy-check", glyphs::COPY_CHECK),
    CopyX => ("copy-x", glyphs::COPY_X),
    TextScanTwo => ("text-scan-two", glyphs::TEXT_SCAN_TWO),
    CameraPlus => ("camera-plus", glyphs::CAMERA_PLUS),
    CalendarPlus => ("calendar-plus", glyphs::CALENDAR_PLUS),
    CalendarMinus => ("calendar-minus", glyphs::CALENDAR_MINUS),
    SearchOff => ("search-off", glyphs::SEARCH_OFF),
    FilterOff => ("filter-off", glyphs::FILTER_OFF),

    // More utilities.
    SortAscending => ("sort-ascending", glyphs::SORT_ASCENDING),
    SortDescending => ("sort-descending", glyphs::SORT_DESCENDING),
    ArrowsSort => ("arrows-sort", glyphs::ARROWS_SORT),
    SettingsAutomation => ("settings-automation", glyphs::SETTINGS_AUTOMATION),
    Adjustments => ("adjustments", glyphs::ADJUSTMENTS),
    AdjustmentsHorizontal => ("adjustments-horizontal", glyphs::ADJUSTMENTS_HORIZONTAL),
    Tool => ("tool", glyphs::TOOL),
    ToolsOff => ("tools-off", glyphs::TOOLS_OFF),
    Wand => ("wand", glyphs::WAND),
    WandOff => ("wand-off", glyphs::WAND_OFF),


}
