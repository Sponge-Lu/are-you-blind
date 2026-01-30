#![windows_subsystem = "windows"]
#![allow(clippy::upper_case_acronyms)] // Windows API types use uppercase names

use rand::seq::SliceRandom;
use slint::{SharedString, Timer, TimerMode};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIconBuilder, TrayIconEvent,
};

slint::include_modules!();

#[cfg(target_os = "windows")]
fn enable_windows_per_monitor_dpi_awareness() {
    use std::ffi::c_void;

    type HMODULE = *mut c_void;
    type FARPROC = *mut c_void;
    type BOOL = i32;
    type HRESULT = i32;

    const PROCESS_PER_MONITOR_DPI_AWARE: i32 = 2;

    fn wide_null_terminated(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(lp_lib_file_name: *const u16) -> HMODULE;
        fn GetProcAddress(h_module: HMODULE, lp_proc_name: *const i8) -> FARPROC;
    }

    unsafe {
        let user32 = LoadLibraryW(wide_null_terminated("user32.dll").as_ptr());
        if !user32.is_null() {
            let set_context = GetProcAddress(user32, c"SetProcessDpiAwarenessContext".as_ptr());
            if !set_context.is_null() {
                type SetProcessDpiAwarenessContextFn =
                    unsafe extern "system" fn(*mut c_void) -> BOOL;
                let set_context: SetProcessDpiAwarenessContextFn = std::mem::transmute(set_context);
                if set_context((-4isize) as *mut c_void) != 0 {
                    return;
                }
            }
        }

        let shcore = LoadLibraryW(wide_null_terminated("shcore.dll").as_ptr());
        if !shcore.is_null() {
            let set_awareness = GetProcAddress(shcore, c"SetProcessDpiAwareness".as_ptr());
            if !set_awareness.is_null() {
                type SetProcessDpiAwarenessFn = unsafe extern "system" fn(i32) -> HRESULT;
                let set_awareness: SetProcessDpiAwarenessFn = std::mem::transmute(set_awareness);
                if set_awareness(PROCESS_PER_MONITOR_DPI_AWARE) == 0 {
                    return;
                }
            }
        }

        if !user32.is_null() {
            let set_dpi_aware = GetProcAddress(user32, c"SetProcessDPIAware".as_ptr());
            if !set_dpi_aware.is_null() {
                type SetProcessDPIAwareFn = unsafe extern "system" fn() -> BOOL;
                let set_dpi_aware: SetProcessDPIAwareFn = std::mem::transmute(set_dpi_aware);
                let _ = set_dpi_aware();
            }
        }
    }
}

struct AppState {
    is_paused: bool,
    work_duration: Duration,
    rest_duration: Duration,
    water_interval: u32, // 每几轮护眼提醒后触发喝水提醒
    walk_interval: u32,  // 每几轮护眼提醒后触发走动提醒
    eye_rest_count: u32, // 当前护眼提醒计数
    current_mode: Mode,
    current_rest_type: RestType, // 当前休息类型
    start_time: Instant,
    last_tick: Instant,
    overlay_windows: Vec<OverlayWindowEntry>,
    main_window_visible: bool,
    drag_anchor_window_pos: Option<slint::LogicalPosition>,
    drag_anchor_pointer_screen_pos: Option<slint::LogicalPosition>,
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Work,
    Rest,
}

#[derive(PartialEq, Clone, Copy)]
enum RestType {
    EyeRest, // 护眼休息
    Water,   // 喝水提醒
    Walk,    // 走动提醒
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_paused: false,
            work_duration: Duration::from_secs(20 * 60),
            rest_duration: Duration::from_secs(20),
            water_interval: 2,
            walk_interval: 3,
            eye_rest_count: 0,
            current_mode: Mode::Work,
            current_rest_type: RestType::EyeRest,
            start_time: Instant::now(),
            last_tick: Instant::now(),
            overlay_windows: Vec::new(),
            main_window_visible: true,
            drag_anchor_window_pos: None,
            drag_anchor_pointer_screen_pos: None,
        }
    }
}

fn format_duration_mm_ss(duration: Duration) -> SharedString {
    let secs_remaining = duration.as_secs();
    let mins = secs_remaining / 60;
    let secs = secs_remaining % 60;
    SharedString::from(format!("{:02}:{:02}", mins, secs))
}

/// 获取随机的护眼休息提示信息
fn get_eye_rest_message(rest_seconds: u64) -> (&'static str, String) {
    let messages: Vec<(&str, &str)> = vec![
        // 20-20-20 法则相关
        ("👀 护眼时间", "20-20-20 法则：每 20 分钟看 20 英尺外 {} 秒"),
        (
            "🌳 远眺时刻",
            "把目光投向窗外最远的地方，让睫状肌放松 {} 秒",
        ),
        ("🏔️ 望远休息", "想象你在山顶眺望远方，眼睛自然放松 {} 秒"),
        // 科普类 - 眼睛健康
        (
            "💡 护眼小知识",
            "人平均每分钟眨眼 15-20 次，专注屏幕时可能降到 3-4 次！休息 {} 秒",
        ),
        (
            "🔬 眼科冷知识",
            "你的眼睛有 200 万个工作部件，是身体最复杂的器官之一。爱护它 {} 秒",
        ),
        (
            "📚 护眼科普",
            "蓝光会抑制褪黑素分泌，影响睡眠。现在休息 {} 秒，让眼睛喘口气",
        ),
        (
            "🧬 眼睛构造",
            "角膜是人体唯一没有血管的组织，全靠泪液供氧。眨眨眼休息 {} 秒",
        ),
        (
            "🌙 夜间护眼",
            "晚上用眼更要注意休息，黑暗中瞳孔扩大更容易疲劳。休息 {} 秒",
        ),
        (
            "📖 近视预防",
            "每天户外活动 2 小时可有效预防近视。先休息 {} 秒吧",
        ),
        (
            "🔍 视力保护",
            "眼睛干涩？可能是泪膜蒸发太快。多眨眼，休息 {} 秒",
        ),
        // 幽默诙谐类
        (
            "🦉 猫头鹰说",
            "即使我能 270° 转头，也需要休息。你的脖子只能转 180°，更该歇歇了！{} 秒",
        ),
        (
            "🐱 喵星人提醒",
            "猫咪每天睡 16 小时都精神抖擞，你才休息 {} 秒有啥不行的？",
        ),
        (
            "🐕 汪星人建议",
            "狗子撒娇要出门遛弯，你的眼睛也想出去看看。休息 {} 秒",
        ),
        (
            "🦅 老鹰视角",
            "老鹰能看清 3 公里外的兔子，全靠好好保护眼睛。休息 {} 秒",
        ),
        (
            "🐸 青蛙观点",
            "井底之蛙：虽然我只看得到井口，但我从不盯着屏幕。休息 {} 秒",
        ),
        (
            "🎮 游戏暂停",
            "就算大神也要暂停存档，你的眼睛也需要 checkpoint！{} 秒",
        ),
        (
            "☕ 程序员定律",
            "while(眼睛疲劳) {{ break; }} // 休息 {} 秒",
        ),
        (
            "🚀 太空护眼",
            "宇航员在太空也要做眼保健操，地球人更该休息 {} 秒",
        ),
        ("🎬 导演喊卡", "导演说：\"卡！\" 眼睛杀青休息 {} 秒"),
        ("🎭 眼睛罢工", "您的眼睛申请了 {} 秒带薪休假，已批准"),
        // 健康危害警示（轻松版）
        (
            "⚠️ 温馨提示",
            "长时间盯屏幕可能导致头痛、肩颈酸痛。起来活动 {} 秒吧",
        ),
        (
            "🏥 眼科医生说",
            "干眼症患者越来越年轻化了，休息 {} 秒预防一下",
        ),
        (
            "💊 不吃药的处方",
            "治疗眼疲劳最好的药：休息 {} 秒 + 远眺绿色植物",
        ),
        (
            "🩺 健康小贴士",
            "眼疲劳会引起头痛，头痛会影响心情，心情差会摸鱼。休息 {} 秒吧",
        ),
        // 激励类
        ("💪 效率提升", "适当休息反而能提高工作效率。科学划水 {} 秒"),
        ("🧠 大脑充电", "让眼睛和大脑同步休息 {} 秒，待会儿更清醒"),
        ("⚡ 能量恢复", "短暂休息能恢复注意力，这 {} 秒是值得的投资"),
        ("🎯 专注重置", "暂停是为了更好地出发。休息 {} 秒，重新聚焦"),
    ];

    let (headline, template) = messages
        .choose(&mut rand::thread_rng())
        .unwrap_or(&("👀 护眼时间", "休息 {} 秒，保护视力"));

    (*headline, template.replace("{}", &rest_seconds.to_string()))
}

/// 获取随机的喝水提示信息
fn get_water_message(rest_seconds: u64) -> (&'static str, String) {
    let messages: Vec<(&str, &str)> = vec![
        // 基础提醒
        ("💧 喝水时间", "起来喝杯水吧！保持身体水分充足（{} 秒）"),
        (
            "🚰 补水提醒",
            "该喝水啦！人体 70% 是水，别让自己\"干涸\"（{} 秒）",
        ),
        ("🥤 饮水时刻", "水是生命之源，现在就喝一杯吧（{} 秒）"),
        // 科普类 - 喝水的重要性
        (
            "📊 健康数据",
            "人每天需要 2000ml 水，你今天喝够了吗？（{} 秒）",
        ),
        (
            "🧪 身体需求",
            "缺水 2% 就会影响注意力和记忆力。快喝水！（{} 秒）",
        ),
        ("🔬 科学喝水", "少量多次喝水比一次猛灌更健康（{} 秒）"),
        ("💡 喝水冷知识", "大脑 75% 是水，缺水会变\"笨\"哦（{} 秒）"),
        ("🌡️ 温度建议", "温水 (35-40°C) 最容易被身体吸收（{} 秒）"),
        ("⏰ 喝水时机", "起床、饭前、运动后是喝水的黄金时间（{} 秒）"),
        // 不喝水的危害
        (
            "⚠️ 缺水警告",
            "缺水会导致头痛、疲劳、皮肤干燥。快补水！（{} 秒）",
        ),
        (
            "🏥 健康提示",
            "长期缺水可能导致肾结石，喝水是最便宜的养生（{} 秒）",
        ),
        (
            "😵 疲劳信号",
            "感觉累？可能不是困，是渴！喝杯水试试（{} 秒）",
        ),
        ("🤯 大脑求救", "头昏脑涨？你的大脑在喊渴！（{} 秒）"),
        // 幽默诙谐类
        (
            "🐫 骆驼都笑了",
            "骆驼：我能 7 天不喝水，你可不行！（{} 秒）",
        ),
        ("🐟 鱼的建议", "我一辈子泡在水里，你至少喝两口吧（{} 秒）"),
        ("🌊 海绵宝宝说", "虽然我住海里，但淡水更健康哦（{} 秒）"),
        (
            "☕ 咖啡警告",
            "咖啡不是水的替代品！喝完咖啡更要补水（{} 秒）",
        ),
        ("🍺 酒精提示", "昨晚喝酒了？今天更要多喝水排毒（{} 秒）"),
        ("🧊 冰水冷知识", "冰水会让胃收缩，温水更舒服哦（{} 秒）"),
        ("🎮 游戏补给", "真正的大神都知道：喝水是最强 buff（{} 秒）"),
        (
            "💻 程序员必备",
            "Coffee++ 不如 Water++，少喝咖啡多喝水（{} 秒）",
        ),
        ("🦴 骨骼精奇", "关节润滑需要水，别让自己\"生锈\"（{} 秒）"),
        // 激励类
        ("✨ 美容秘方", "多喝水是最便宜的护肤品（{} 秒）"),
        ("🏃 代谢加速", "喝水能促进新陈代谢，助力减脂（{} 秒）"),
        ("🎯 效率提升", "充足饮水能让你保持清醒专注（{} 秒）"),
    ];

    let (headline, template) = messages
        .choose(&mut rand::thread_rng())
        .unwrap_or(&("💧 喝水时间", "起来喝杯水吧（{} 秒）"));

    (*headline, template.replace("{}", &rest_seconds.to_string()))
}

/// 获取随机的走动提示信息
fn get_walk_message(rest_seconds: u64) -> (&'static str, String) {
    let messages: Vec<(&str, &str)> = vec![
        // 基础提醒
        ("🚶 走动时间", "站起来活动一下身体！（{} 秒）"),
        ("🏃 运动时刻", "久坐是健康杀手，起来动动吧（{} 秒）"),
        ("🧘 伸展提醒", "伸个懒腰，活动筋骨（{} 秒）"),
        // 科普类 - 久坐危害
        (
            "📊 久坐数据",
            "久坐超过 1 小时，预期寿命减少 22 分钟！（{} 秒）",
        ),
        ("🔬 科学发现", "久坐会导致血液循环变慢，快起来走走（{} 秒）"),
        ("🏥 医学警告", "久坐是\"新型吸烟\"，同样危害健康（{} 秒）"),
        (
            "💡 健康知识",
            "每坐 30 分钟起来活动 2 分钟，可以抵消久坐伤害（{} 秒）",
        ),
        (
            "🦴 骨骼健康",
            "久坐会让骨密度降低，多走动才能保持骨骼健康（{} 秒）",
        ),
        (
            "🫀 心脏提醒",
            "久坐让心血管疾病风险增加 147%！起来活动（{} 秒）",
        ),
        ("🧠 大脑供血", "站起来能增加大脑供血，思路更清晰（{} 秒）"),
        // 身体部位提醒
        ("🦵 腿部呼救", "你的腿想念走路的感觉了！（{} 秒）"),
        (
            "🦴 脊椎请求",
            "你的脊椎承受了很大压力，让它休息一下（{} 秒）",
        ),
        ("💪 肌肉松弛", "久坐让肌肉萎缩，起来激活它们（{} 秒）"),
        (
            "🤸 关节润滑",
            "关节需要运动来分泌润滑液，别让它们\"生锈\"（{} 秒）",
        ),
        ("👣 脚趾活动", "动动脚趾，促进下肢血液循环（{} 秒）"),
        // 幽默诙谐类
        ("🐢 乌龟都着急", "连乌龟都比你动得多，起来走走！（{} 秒）"),
        ("🦥 树懒震惊", "树懒：没想到有人比我还懒！（{} 秒）"),
        ("🪑 椅子抗议", "你的椅子申请轮换休息了（{} 秒）"),
        ("🍑 屁股抗议", "久坐让屁股变扁，不信你摸摸（{} 秒）"),
        ("🐕 遛狗时间", "就算没有狗，也可以假装遛自己（{} 秒）"),
        (
            "🚀 宇航员训练",
            "NASA 要求宇航员每天运动 2 小时，你先动 {} 秒",
        ),
        ("🏋️ 健身房欠费", "办了健身卡不去，不如先站起来（{} 秒）"),
        ("🎮 角色需要走位", "现实也要走位！别只会在游戏里跑（{} 秒）"),
        (
            "📱 步数挑战",
            "微信运动 100 步也是步数，起来贡献一下（{} 秒）",
        ),
        // 建议动作
        ("🤸 推荐动作", "试试原地高抬腿，激活下肢肌肉（{} 秒）"),
        ("🧘 办公室瑜伽", "站起来做几个深蹲，唤醒臀部肌肉（{} 秒）"),
        ("💃 扭一扭", "扭扭腰，转转头，活动全身关节（{} 秒）"),
        ("🏃 小跑一下", "绕办公室走一圈，或原地踏步（{} 秒）"),
        ("🙆 伸展运动", "双手举过头顶，做个全身伸展（{} 秒）"),
        // 激励类
        ("⚡ 能量激活", "活动一下，血液循环加速，精力充沛（{} 秒）"),
        ("🎯 效率秘诀", "适当活动能让下午不犯困（{} 秒）"),
        ("✨ 健康投资", "每天多走 2000 步，一年下来了不起（{} 秒）"),
    ];

    let (headline, template) = messages
        .choose(&mut rand::thread_rng())
        .unwrap_or(&("🚶 走动时间", "站起来活动一下身体（{} 秒）"));

    (*headline, template.replace("{}", &rest_seconds.to_string()))
}

struct OverlayWindowEntry {
    window: RestOverlayWindow,
    #[cfg(target_os = "windows")]
    monitor: MonitorRect,
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct MonitorRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale_factor: f32,
}

#[cfg(target_os = "windows")]
fn monitor_rects() -> Vec<MonitorRect> {
    use std::ffi::c_void;
    use std::mem::MaybeUninit;
    use std::ptr;

    type HMONITOR = *mut c_void;
    type HDC = *mut c_void;
    type LPARAM = isize;
    type BOOL = i32;
    type UINT = u32;
    type HRESULT = i32;
    const CCHDEVICENAME: usize = 32;
    const CCHFORMNAME: usize = 32;
    const ENUM_CURRENT_SETTINGS: u32 = 0xFFFF_FFFF;
    const MDT_EFFECTIVE_DPI: i32 = 0;

    #[repr(C)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct MONITORINFOEXW {
        cbSize: u32,
        rcMonitor: RECT,
        rcWork: RECT,
        dwFlags: u32,
        szDevice: [u16; CCHDEVICENAME],
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct POINTL {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct DEVMODEW {
        dmDeviceName: [u16; CCHDEVICENAME],
        dmSpecVersion: u16,
        dmDriverVersion: u16,
        dmSize: u16,
        dmDriverExtra: u16,
        dmFields: u32,
        dmPosition: POINTL,
        dmDisplayOrientation: u32,
        dmDisplayFixedOutput: u32,
        dmColor: i16,
        dmDuplex: i16,
        dmYResolution: i16,
        dmTTOption: i16,
        dmCollate: i16,
        dmFormName: [u16; CCHFORMNAME],
        dmLogPixels: u16,
        dmBitsPerPel: u32,
        dmPelsWidth: u32,
        dmPelsHeight: u32,
        dmDisplayFlags: u32,
        dmDisplayFrequency: u32,
        dmICMMethod: u32,
        dmICMIntent: u32,
        dmMediaType: u32,
        dmDitherType: u32,
        dmReserved1: u32,
        dmReserved2: u32,
        dmPanningWidth: u32,
        dmPanningHeight: u32,
    }

    type MonitorEnumProc =
        Option<unsafe extern "system" fn(HMONITOR, HDC, *mut RECT, LPARAM) -> BOOL>;

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayMonitors(
            hdc: HDC,
            lprcClip: *const RECT,
            lpfnEnum: MonitorEnumProc,
            dwData: LPARAM,
        ) -> BOOL;
        fn GetMonitorInfoW(hMonitor: HMONITOR, lpmi: *mut MONITORINFOEXW) -> BOOL;
        fn EnumDisplaySettingsW(
            lpszDeviceName: *const u16,
            iModeNum: u32,
            lpDevMode: *mut DEVMODEW,
        ) -> BOOL;
    }

    #[link(name = "Shcore")]
    extern "system" {
        fn GetDpiForMonitor(
            hmonitor: HMONITOR,
            dpiType: i32,
            dpiX: *mut UINT,
            dpiY: *mut UINT,
        ) -> HRESULT;
    }

    unsafe extern "system" fn enum_monitor_cb(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let monitors: &mut Vec<MonitorRect> = &mut *(data as *mut Vec<MonitorRect>);

        let mut mi = MaybeUninit::<MONITORINFOEXW>::zeroed();
        (*mi.as_mut_ptr()).cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmonitor, mi.as_mut_ptr()) != 0 {
            let mi = mi.assume_init();

            let mut rect: Option<MonitorRect> = None;

            let mut scale_factor = 1.0f32;
            let mut dpi_x: UINT = 0;
            let mut dpi_y: UINT = 0;
            if GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == 0
                && dpi_x > 0
            {
                scale_factor = dpi_x as f32 / 96.0;
            }

            let mut dm = MaybeUninit::<DEVMODEW>::zeroed();
            (*dm.as_mut_ptr()).dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(mi.szDevice.as_ptr(), ENUM_CURRENT_SETTINGS, dm.as_mut_ptr())
                != 0
            {
                let dm = dm.assume_init();
                rect = Some(MonitorRect {
                    x: dm.dmPosition.x,
                    y: dm.dmPosition.y,
                    width: dm.dmPelsWidth,
                    height: dm.dmPelsHeight,
                    scale_factor,
                });
            } else {
                let r = mi.rcMonitor;
                let width = (r.right - r.left).max(0) as u32;
                let height = (r.bottom - r.top).max(0) as u32;
                if width > 0 && height > 0 {
                    rect = Some(MonitorRect {
                        x: r.left,
                        y: r.top,
                        width,
                        height,
                        scale_factor,
                    });
                }
            }

            if let Some(rect) = rect {
                if rect.width > 0 && rect.height > 0 {
                    monitors.push(rect);
                }
            }
        }

        1
    }

    let mut monitors = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null(),
            Some(enum_monitor_cb),
            (&mut monitors as *mut Vec<MonitorRect>) as LPARAM,
        );
    }
    monitors
}

#[cfg(target_os = "windows")]
fn fit_overlay_to_monitor(entry: &OverlayWindowEntry) {
    let window = entry.window.window();
    window.set_position(slint::PhysicalPosition::new(
        entry.monitor.x,
        entry.monitor.y,
    ));

    let scale_factor = window.scale_factor().max(0.1);
    let logical_width = (entry.monitor.width as f32) / scale_factor + 1.0;
    let logical_height = (entry.monitor.height as f32) / scale_factor + 1.0;

    window.set_size(slint::LogicalSize::new(logical_width, logical_height));
}

#[cfg(target_os = "windows")]
fn virtual_screen_rect() -> MonitorRect {
    use std::ffi::c_int;

    #[link(name = "user32")]
    extern "system" {
        fn GetSystemMetrics(nIndex: c_int) -> c_int;
    }

    const SM_XVIRTUALSCREEN: c_int = 76;
    const SM_YVIRTUALSCREEN: c_int = 77;
    const SM_CXVIRTUALSCREEN: c_int = 78;
    const SM_CYVIRTUALSCREEN: c_int = 79;

    let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(1) as u32;
    let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(1) as u32;

    MonitorRect {
        x,
        y,
        width,
        height,
        scale_factor: 1.0,
    }
}

fn show_rest_overlay(state: &mut AppState, remaining: Duration, headline: &str, message: &str) {
    let headline: SharedString = headline.into();
    let message: SharedString = message.into();
    let countdown = format_duration_mm_ss(remaining);

    // Always recreate overlay windows to handle monitor changes
    state.overlay_windows.clear();

    #[cfg(target_os = "windows")]
    {
        for m in monitor_rects() {
            let Ok(overlay) = RestOverlayWindow::new() else {
                continue;
            };

            let entry = OverlayWindowEntry {
                window: overlay,
                monitor: m,
            };
            fit_overlay_to_monitor(&entry);
            state.overlay_windows.push(entry);
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(overlay) = RestOverlayWindow::new() {
            overlay.window().set_fullscreen(true);
            state
                .overlay_windows
                .push(OverlayWindowEntry { window: overlay });
        }
    }

    // Fallback
    if state.overlay_windows.is_empty() {
        #[cfg(target_os = "windows")]
        {
            if let Ok(overlay) = RestOverlayWindow::new() {
                let monitor = monitor_rects()
                    .into_iter()
                    .next()
                    .unwrap_or_else(virtual_screen_rect);
                let entry = OverlayWindowEntry {
                    window: overlay,
                    monitor,
                };
                fit_overlay_to_monitor(&entry);
                state.overlay_windows.push(entry);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(overlay) = RestOverlayWindow::new() {
                overlay.window().set_fullscreen(true);
                state
                    .overlay_windows
                    .push(OverlayWindowEntry { window: overlay });
            }
        }
    }

    for overlay in &state.overlay_windows {
        overlay.window.set_headline(headline.clone());
        overlay.window.set_message(message.clone());
        overlay.window.set_countdown(countdown.clone());

        #[cfg(target_os = "windows")]
        fit_overlay_to_monitor(overlay);

        let _ = overlay.window.window().show();

        #[cfg(target_os = "windows")]
        fit_overlay_to_monitor(overlay);

        overlay.window.window().request_redraw();
    }
}

fn update_rest_overlay(state: &mut AppState, remaining: Duration) {
    if state.overlay_windows.is_empty() {
        return;
    }

    let countdown = format_duration_mm_ss(remaining);
    for overlay in &state.overlay_windows {
        overlay.window.set_countdown(countdown.clone());

        #[cfg(target_os = "windows")]
        fit_overlay_to_monitor(overlay);

        overlay.window.window().request_redraw();
    }
}

fn hide_rest_overlay(state: &mut AppState) {
    for overlay in &state.overlay_windows {
        let _ = overlay.window.window().hide();
    }
    state.overlay_windows.clear();
}

/// Load tray icon from embedded PNG data
fn create_tray_icon() -> tray_icon::Icon {
    // Embed the logo PNG at compile time
    let png_data = include_bytes!("../assets/tray-icon.png");
    let img = image::load_from_memory(png_data).expect("Failed to load tray icon");
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), width, height).expect("Failed to create tray icon")
}

fn main() -> Result<(), slint::PlatformError> {
    #[cfg(target_os = "windows")]
    enable_windows_per_monitor_dpi_awareness();

    let main_window = MainWindow::new()?;
    let state = Rc::new(RefCell::new(AppState::default()));

    // Create system tray menu
    let menu = Menu::new();
    let show_item = MenuItem::new("显示窗口", true, None);
    let quit_item = MenuItem::new("退出", true, None);
    menu.append_items(&[&show_item, &quit_item]).unwrap();

    // Create system tray icon
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("瞎了么")
        .with_icon(create_tray_icon())
        .build()
        .expect("Failed to create tray icon");

    // Store menu item IDs for event handling
    let show_item_id = show_item.id().clone();
    let quit_item_id = quit_item.id().clone();

    // Sync initial settings UI
    main_window.set_work_minutes((state.borrow().work_duration.as_secs() / 60) as i32);
    main_window.set_rest_seconds(state.borrow().rest_duration.as_secs() as i32);
    main_window.set_water_interval(state.borrow().water_interval as i32);
    main_window.set_walk_interval(state.borrow().walk_interval as i32);

    // Main timer for countdown logic
    let timer = Timer::default();
    let state_timer = state.clone();
    let main_weak = main_window.as_weak();

    timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        let mut state = state_timer.borrow_mut();

        if state.is_paused {
            let paused_for = state.last_tick.elapsed();
            state.start_time += paused_for;
            state.last_tick = Instant::now();
            return;
        }

        state.last_tick = Instant::now();
        let app = match main_weak.upgrade() {
            Some(ui) => ui,
            None => return,
        };

        let elapsed = state.start_time.elapsed();
        let limit = match state.current_mode {
            Mode::Work => state.work_duration,
            Mode::Rest => state.rest_duration,
        };

        if elapsed >= limit {
            state.start_time = Instant::now();
            match state.current_mode {
                Mode::Work => {
                    state.current_mode = Mode::Rest;
                    state.eye_rest_count += 1;
                    let rest_duration = state.rest_duration;
                    let count = state.eye_rest_count;

                    // 判断是否需要额外提醒：走动 > 喝水（优先级）
                    state.current_rest_type = if count % state.walk_interval == 0 {
                        RestType::Walk
                    } else if count % state.water_interval == 0 {
                        RestType::Water
                    } else {
                        RestType::EyeRest
                    };

                    // 护眼提示始终显示（核心功能）
                    let (headline, mut message) = get_eye_rest_message(rest_duration.as_secs());

                    // 如果需要喝水或走动，附加额外提示
                    match state.current_rest_type {
                        RestType::Water => {
                            let (_, water_msg) = get_water_message(rest_duration.as_secs());
                            message = format!("{}\n\n💧 顺便提醒：{}", message, water_msg);
                        }
                        RestType::Walk => {
                            let (_, walk_msg) = get_walk_message(rest_duration.as_secs());
                            message = format!("{}\n\n🚶 顺便提醒：{}", message, walk_msg);
                        }
                        RestType::EyeRest => {}
                    }

                    // Hide main window during rest
                    if state.main_window_visible {
                        let _ = app.window().hide();
                    }

                    show_rest_overlay(&mut state, rest_duration, headline, &message);
                    app.set_status_text("Rest your eyes!".into());
                    app.set_time_display(format_duration_mm_ss(state.rest_duration));
                    app.set_progress(1.0);
                }
                Mode::Rest => {
                    state.current_mode = Mode::Work;
                    hide_rest_overlay(&mut state);

                    // Always show main window after rest
                    let _ = app.window().show();
                    state.main_window_visible = true;

                    app.set_status_text("Focus Time".into());
                    app.set_time_display(format_duration_mm_ss(state.work_duration));
                    app.set_progress(1.0);
                }
            }
        } else {
            let remaining = limit - elapsed;
            let secs_remaining = remaining.as_secs();
            let mins = secs_remaining / 60;
            let secs = secs_remaining % 60;

            app.set_time_display(SharedString::from(format!("{:02}:{:02}", mins, secs)));

            let progress = 1.0 - (elapsed.as_secs_f32() / limit.as_secs_f32());
            app.set_progress(progress);

            if state.current_mode == Mode::Rest {
                update_rest_overlay(&mut state, remaining);
            }
        }
    });

    // Timer for polling tray events
    let tray_timer = Timer::default();
    let state_tray = state.clone();
    let main_weak_tray = main_window.as_weak();
    let show_id = show_item_id.clone();
    let quit_id = quit_item_id.clone();

    tray_timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        // Handle menu events
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == show_id {
                // Show main window
                if let Some(app) = main_weak_tray.upgrade() {
                    let _ = app.window().show();
                    state_tray.borrow_mut().main_window_visible = true;
                }
            } else if event.id == quit_id {
                // Quit application
                slint::quit_event_loop().ok();
            }
        }

        // Handle tray icon click events
        if let Ok(TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            ..
        }) = TrayIconEvent::receiver().try_recv()
        {
            // Left click: show main window
            if let Some(app) = main_weak_tray.upgrade() {
                let _ = app.window().show();
                state_tray.borrow_mut().main_window_visible = true;
            }
        }
    });

    // Toggle timer callback
    let state_toggle = state.clone();
    let main_weak_toggle = main_window.as_weak();
    main_window.on_toggle_timer(move || {
        let mut state = state_toggle.borrow_mut();
        state.is_paused = !state.is_paused;
        if let Some(app) = main_weak_toggle.upgrade() {
            app.set_is_paused(state.is_paused);
        }
    });

    // Secondary action (reset/skip) callback
    let state_secondary = state.clone();
    let main_weak_secondary = main_window.as_weak();
    main_window.on_secondary_action(move || {
        let mut state = state_secondary.borrow_mut();
        state.start_time = Instant::now();
        if let Some(app) = main_weak_secondary.upgrade() {
            match state.current_mode {
                Mode::Work => {
                    app.set_time_display(format_duration_mm_ss(state.work_duration));
                    app.set_progress(1.0);
                }
                Mode::Rest => {
                    state.current_mode = Mode::Work;
                    hide_rest_overlay(&mut state);
                    let _ = app.window().show();
                    state.main_window_visible = true;
                    app.set_status_text("Focus Time".into());
                    app.set_time_display(format_duration_mm_ss(state.work_duration));
                    app.set_progress(1.0);
                }
            }
        }
    });

    // Apply work minutes callback
    let state_apply_minutes = state.clone();
    let main_weak_apply_minutes = main_window.as_weak();
    main_window.on_apply_work_minutes(move |minutes| {
        let minutes = minutes.clamp(1, 180);
        let mut state = state_apply_minutes.borrow_mut();
        state.work_duration = Duration::from_secs(minutes as u64 * 60);
        if state.current_mode == Mode::Work {
            state.start_time = Instant::now();
            state.last_tick = Instant::now();
        }

        if let Some(app) = main_weak_apply_minutes.upgrade() {
            app.set_work_minutes(minutes);
            if state.current_mode == Mode::Work {
                app.set_status_text("Focus Time".into());
                app.set_time_display(format_duration_mm_ss(state.work_duration));
                app.set_progress(1.0);
            }
        }
    });

    // Apply rest seconds callback
    let state_apply_rest = state.clone();
    let main_weak_apply_rest = main_window.as_weak();
    main_window.on_apply_rest_seconds(move |seconds| {
        let seconds = seconds.clamp(5, 300);
        let mut state = state_apply_rest.borrow_mut();
        state.rest_duration = Duration::from_secs(seconds as u64);

        if let Some(app) = main_weak_apply_rest.upgrade() {
            app.set_rest_seconds(seconds);
        }
    });

    // Apply water interval callback
    let state_apply_water = state.clone();
    let main_weak_apply_water = main_window.as_weak();
    main_window.on_apply_water_interval(move |interval| {
        let interval = interval.clamp(1, 20);
        let mut state = state_apply_water.borrow_mut();
        state.water_interval = interval as u32;

        if let Some(app) = main_weak_apply_water.upgrade() {
            app.set_water_interval(interval);
        }
    });

    // Apply walk interval callback
    let state_apply_walk = state.clone();
    let main_weak_apply_walk = main_window.as_weak();
    main_window.on_apply_walk_interval(move |interval| {
        let interval = interval.clamp(1, 20);
        let mut state = state_apply_walk.borrow_mut();
        state.walk_interval = interval as u32;

        if let Some(app) = main_weak_apply_walk.upgrade() {
            app.set_walk_interval(interval);
        }
    });

    // Window drag callbacks
    let state_drag_start = state.clone();
    let main_weak_drag_start = main_window.as_weak();
    main_window.on_start_window_drag(move |position| {
        let Some(app) = main_weak_drag_start.upgrade() else {
            return;
        };

        let window = app.window();
        let window_pos = window.position().to_logical(window.scale_factor());
        let pointer_screen_pos =
            slint::LogicalPosition::new(window_pos.x + position.x, window_pos.y + position.y);

        let mut state = state_drag_start.borrow_mut();
        state.drag_anchor_window_pos = Some(window_pos);
        state.drag_anchor_pointer_screen_pos = Some(pointer_screen_pos);
    });

    let state_drag_update = state.clone();
    let main_weak_drag_update = main_window.as_weak();
    main_window.on_update_window_drag(move |position| {
        let Some(app) = main_weak_drag_update.upgrade() else {
            return;
        };

        let window = app.window();
        let window_pos = window.position().to_logical(window.scale_factor());
        let pointer_screen_pos =
            slint::LogicalPosition::new(window_pos.x + position.x, window_pos.y + position.y);

        let state = state_drag_update.borrow();
        let Some(anchor_window_pos) = state.drag_anchor_window_pos else {
            return;
        };
        let Some(anchor_pointer_screen_pos) = state.drag_anchor_pointer_screen_pos else {
            return;
        };

        let delta_x = pointer_screen_pos.x - anchor_pointer_screen_pos.x;
        let delta_y = pointer_screen_pos.y - anchor_pointer_screen_pos.y;
        window.set_position(slint::LogicalPosition::new(
            anchor_window_pos.x + delta_x,
            anchor_window_pos.y + delta_y,
        ));
    });

    let state_drag_end = state.clone();
    main_window.on_end_window_drag(move || {
        let mut state = state_drag_end.borrow_mut();
        state.drag_anchor_window_pos = None;
        state.drag_anchor_pointer_screen_pos = None;
    });

    // Minimize to tray callback (X button)
    let main_weak_min = main_window.as_weak();
    let state_min = state.clone();
    main_window.on_minimize_to_tray(move || {
        if let Some(app) = main_weak_min.upgrade() {
            let _ = app.window().hide();
            state_min.borrow_mut().main_window_visible = false;
        }
    });

    main_window.on_open_settings(move || {
        // Settings panel is handled in Slint UI
    });

    // Show main window and run event loop until quit
    main_window.show()?;

    // Use run_event_loop_until_quit which doesn't exit when all windows are hidden.
    // The timers we created above will keep the event loop alive.
    slint::run_event_loop_until_quit()?;

    Ok(())
}
