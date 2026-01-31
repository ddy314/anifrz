use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table, Tabs, Wrap,
    },
    Frame, Terminal,
};

fn main() -> anyhow::Result<()> {
    // terminal init
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    // restore
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> anyhow::Result<()> {
    let mut app = App::new_demo();
    let tick_rate = Duration::from_millis(120);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    if handle_key(&mut app, k) {
                        break;
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
    Ok(())
}

/* ----------------------------- App State ----------------------------- */

// 现代配色方案
mod colors {
    use ratatui::style::Color;
    
    // 主题色
    pub const PRIMARY: Color = Color::Rgb(138, 180, 248);      // 柔和蓝色
    pub const SECONDARY: Color = Color::Rgb(166, 227, 161);    // 柔和绿色
    pub const ACCENT: Color = Color::Rgb(243, 139, 168);       // 柔和粉色
    pub const WARNING: Color = Color::Rgb(249, 226, 175);      // 柔和黄色
    
    // 状态色
    pub const SUCCESS: Color = Color::Rgb(166, 227, 161);
    pub const ERROR: Color = Color::Rgb(243, 139, 168);
    pub const INFO: Color = Color::Rgb(137, 220, 235);
    
    // 文字色
    pub const TEXT_PRIMARY: Color = Color::Rgb(205, 214, 244);
    pub const TEXT_DIM: Color = Color::Rgb(127, 132, 156);
    pub const TEXT_HIGHLIGHT: Color = Color::Rgb(245, 224, 220);
    
    // 背景色
    pub const BG_DARK: Color = Color::Rgb(30, 30, 46);
    pub const BG_MEDIUM: Color = Color::Rgb(49, 50, 68);
    pub const BG_LIGHT: Color = Color::Rgb(69, 71, 90);
    
    // 边框色
    pub const BORDER: Color = Color::Rgb(116, 199, 236);
    pub const BORDER_DIM: Color = Color::Rgb(88, 91, 112);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Home,
    Library,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LibraryFilter {
    All,
    LocalOnly,
    WantOnly,
    MissingOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetailMode {
    SplitRight,
    OverlayModal,
}

#[derive(Clone, Debug)]
struct Episode {
    no: u32,
    title: String,
    watched: bool,
    local: bool,
    downloading: bool,
    download_started: Option<Instant>,
}

#[derive(Clone, Debug)]
struct Show {
    id: usize,
    title: String,
    title_cn: String,
    year: i32,
    eps_total: u32,
    rating: f32,
    tags: Vec<&'static str>,
    summary: &'static str,

    want: bool,  // “想看”
    local: bool, // 本地是否存在（至少有部分文件）
    episodes: Vec<Episode>,
}

#[derive(Clone, Debug)]
struct RecentEntry {
    show_id: usize,
    ep_no: u32,
    when_label: &'static str, // 纯展示：例如 “今天 / 昨天”
}

struct Toast {
    text: String,
    until: Instant,
}

struct App {
    tab: Tab,

    // data
    shows: Vec<Show>,
    recent: Vec<RecentEntry>,

    // list selections
    home_state: ListState,
    library_state: ListState,
    search_state: ListState,
    detail_ep_state: ratatui::widgets::TableState,

    // library filter
    lib_filter: LibraryFilter,

    // search
    search_query: String,
    search_focus: bool,

    // detail view
    detail_open: bool,
    detail_show_id: Option<usize>,
    detail_mode: DetailMode,

    toast: Option<Toast>,
    
    // 动画计数器
    tick_count: u32,
}

impl App {
    fn new_demo() -> Self {
        let mut shows = demo_shows();
        // 保证一些缺失/想看/本地状态有对比
        // show[0]: 本地+看过
        // show[1]: 想看但本地没有
        // show[2]: 本地但缺几集
        // show[3]: 想看+本地部分
        // show[4]: 本地全有
        // show[5]: 想看但本地没有

        let recent = vec![
            RecentEntry {
                show_id: shows[0].id,
                ep_no: 9,
                when_label: "今天",
            },
            RecentEntry {
                show_id: shows[2].id,
                ep_no: 3,
                when_label: "昨天",
            },
            RecentEntry {
                show_id: shows[4].id,
                ep_no: 1,
                when_label: "上周",
            },
        ];

        // home default select
        let mut home_state = ListState::default();
        home_state.select(Some(0));
        let mut library_state = ListState::default();
        library_state.select(Some(0));
        let mut search_state = ListState::default();
        search_state.select(Some(0));
        let mut detail_ep_state = ListState::default();
        detail_ep_state.select(Some(0));

        // 给一些 watched 标记
        let id0 = shows[0].id;
        let id2 = shows[2].id;
        let id4 = shows[4].id;
        mark_watched(&mut shows, id0, 1..=9);
        mark_watched(&mut shows, id2, 1..=3);
        mark_watched(&mut shows, id4, 1..=1);

        let mut detail_ep_state = ratatui::widgets::TableState::default();
        detail_ep_state.select(Some(0));

        let mut detail_ep_state = ratatui::widgets::TableState::default();
        detail_ep_state.select(Some(0));

        Self {
            tab: Tab::Home,
            shows,
            recent,

            home_state,
            library_state,
            search_state,
            detail_ep_state,

            lib_filter: LibraryFilter::All,

            search_query: String::new(),
            search_focus: false,

            detail_open: false,
            detail_show_id: None,
            detail_mode: DetailMode::SplitRight,

            toast: None,
            tick_count: 0,
        }
    }

    fn show_toast(&mut self, s: impl Into<String>) {
        self.toast = Some(Toast {
            text: s.into(),
            until: Instant::now() + Duration::from_secs(2),
        });
    }

    fn on_tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        
        // toast expiry
        if let Some(t) = &self.toast {
            if Instant::now() > t.until {
                self.toast = None;
            }
        }

        // simulate downloads: 2s -> local
        for show in &mut self.shows {
            for ep in &mut show.episodes {
                if ep.downloading {
                    if let Some(st) = ep.download_started {
                        if st.elapsed() >= Duration::from_secs(2) {
                            ep.downloading = false;
                            ep.download_started = None;
                            ep.local = true;
                            show.local = true;
                        }
                    }
                }
            }
        }
    }

    fn open_detail(&mut self, show_id: usize) {
        self.detail_open = true;
        self.detail_show_id = Some(show_id);

        let mut ep_state = ratatui::widgets::TableState::default();
        ep_state.select(Some(0));
        self.detail_ep_state = ep_state;
    }

    fn close_detail(&mut self) {
        self.detail_open = false;
        self.detail_show_id = None;
    }

    fn current_show_id_in_tab(&self) -> Option<usize> {
        match self.tab {
            Tab::Home => {
                let idx = self.home_state.selected()?;
                let entry = self.recent.get(idx)?;
                Some(entry.show_id)
            }
            Tab::Library => {
                let filtered = self.filtered_library_ids();
                let idx = self.library_state.selected()?;
                filtered.get(idx).copied()
            }
            Tab::Search => {
                let filtered = self.filtered_search_ids();
                let idx = self.search_state.selected()?;
                filtered.get(idx).copied()
            }
        }
    }

    fn filtered_library_ids(&self) -> Vec<usize> {
        self.shows
            .iter()
            .filter(|s| match self.lib_filter {
                LibraryFilter::All => true,
                LibraryFilter::LocalOnly => s.local,
                LibraryFilter::WantOnly => s.want,
                LibraryFilter::MissingOnly => s.want && !s.local,
            })
            .map(|s| s.id)
            .collect()
    }

    fn filtered_search_ids(&self) -> Vec<usize> {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            return self.shows.iter().map(|s| s.id).collect();
        }
        self.shows
            .iter()
            .filter(|s| {
                s.title.to_lowercase().contains(&q) || s.title_cn.to_lowercase().contains(&q)
            })
            .map(|s| s.id)
            .collect()
    }

    fn show_by_id(&self, id: usize) -> Option<&Show> {
        self.shows.iter().find(|s| s.id == id)
    }
    fn show_by_id_mut(&mut self, id: usize) -> Option<&mut Show> {
        self.shows.iter_mut().find(|s| s.id == id)
    }

    fn selected_episode_index(&self) -> Option<usize> {
        self.detail_ep_state.selected()
    }
}

/* ----------------------------- Key Handling ----------------------------- */

/// return true => quit
fn handle_key(app: &mut App, k: KeyEvent) -> bool {
    // global quit
    if k.code == KeyCode::Char('q') && k.modifiers.is_empty() {
        // 如果开了详情页，q 先关详情
        if app.detail_open {
            app.close_detail();
            return false;
        }
        return true;
    }

    // ESC：关闭详情 / 退出搜索输入
    if k.code == KeyCode::Esc {
        if app.search_focus {
            app.search_focus = false;
            app.show_toast("退出搜索输入");
            return false;
        }
        if app.detail_open {
            app.close_detail();
            return false;
        }
    }

    // 详情页内的按键优先级最高
    if app.detail_open {
        match k.code {
            KeyCode::Char('v') => {
                app.detail_mode = match app.detail_mode {
                    DetailMode::SplitRight => DetailMode::OverlayModal,
                    DetailMode::OverlayModal => DetailMode::SplitRight,
                };
                app.show_toast(match app.detail_mode {
                    DetailMode::SplitRight => "详情：右侧分屏",
                    DetailMode::OverlayModal => "详情：覆盖弹窗",
                });
            }
            KeyCode::Up => table_up(&mut app.detail_ep_state),
            KeyCode::Down => {
                let len = current_episode_len(app);
                table_down(&mut app.detail_ep_state, len);
            }
            KeyCode::Char('w') => {
                if let Some(show_id) = app.detail_show_id {
                    if let Some(ep_idx) = app.selected_episode_index() {
                        if let Some(s) = app.show_by_id_mut(show_id) {
                            if let Some(ep) = s.episodes.get_mut(ep_idx) {
                                ep.watched = !ep.watched;
                                let msg = if ep.watched { "标记：已看" } else { "标记：未看" };
                                app.show_toast(msg);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                // “下载”当前缺失的集
                if let Some(show_id) = app.detail_show_id {
                    if let Some(ep_idx) = app.selected_episode_index() {
                        if let Some(s) = app.show_by_id_mut(show_id) {
                            if let Some(ep) = s.episodes.get_mut(ep_idx) {
                                if ep.local {
                                    app.show_toast("本地已存在：无需下载");
                                } else if ep.downloading {
                                    app.show_toast("正在下载中…");
                                } else {
                                    ep.downloading = true;
                                    ep.download_started = Some(Instant::now());
                                    app.show_toast("开始下载（演示：2 秒后完成）");
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Enter => {
                // Enter 在详情页里：播放（演示）
                app.show_toast("播放：演示（这里会 spawn 外部播放器）");
            }
            _ => {}
        }
        return false;
    }

    // tab switching
    match k.code {
        KeyCode::Char('h') => app.tab = prev_tab(app.tab),
        KeyCode::Char('l') => app.tab = next_tab(app.tab),
        KeyCode::Tab => app.tab = next_tab(app.tab),
        KeyCode::BackTab => app.tab = prev_tab(app.tab),
        _ => {}
    }

    match app.tab {
        Tab::Home => match k.code {
            KeyCode::Up => list_up(&mut app.home_state),
            KeyCode::Down => list_down(&mut app.home_state, app.recent.len()),
            KeyCode::Enter => {
                if let Some(show_id) = app.current_show_id_in_tab() {
                    app.open_detail(show_id);
                }
            }
            _ => {}
        },
        Tab::Library => match k.code {
            KeyCode::Up => list_up(&mut app.library_state),
            KeyCode::Down => {
                let len = app.filtered_library_ids().len();
                list_down(&mut app.library_state, len);
            }
            KeyCode::Char('f') => {
                app.lib_filter = next_filter(app.lib_filter);
                // 切换筛选后把选中位置校正
                let len = app.filtered_library_ids().len();
                if len == 0 {
                    app.library_state.select(None);
                } else {
                    let sel = app.library_state.selected().unwrap_or(0).min(len - 1);
                    app.library_state.select(Some(sel));
                }
                app.show_toast(format!(
                    "筛选：{}",
                    match app.lib_filter {
                        LibraryFilter::All => "全部",
                        LibraryFilter::LocalOnly => "仅本地",
                        LibraryFilter::WantOnly => "仅想看",
                        LibraryFilter::MissingOnly => "想看但缺失",
                    }
                ));
            }
            KeyCode::Enter => {
                if let Some(show_id) = app.current_show_id_in_tab() {
                    app.open_detail(show_id);
                }
            }
            _ => {}
        },
        Tab::Search => match k.code {
            KeyCode::Char('/') => {
                app.search_focus = true;
                app.show_toast("进入搜索输入：直接打字");
            }
            KeyCode::Up => {
                let len = app.filtered_search_ids().len();
                if len > 0 {
                    list_up(&mut app.search_state);
                }
            }
            KeyCode::Down => {
                let len = app.filtered_search_ids().len();
                list_down(&mut app.search_state, len);
            }
            KeyCode::Enter => {
                if app.search_focus {
                    app.search_focus = false;
                    app.show_toast("搜索完成：用上下键选结果，回车进详情");
                } else if let Some(show_id) = app.current_show_id_in_tab() {
                    app.open_detail(show_id);
                }
            }
            KeyCode::Backspace => {
                if app.search_focus {
                    app.search_query.pop();
                }
            }
            KeyCode::Char(c) => {
                if app.search_focus {
                    // Ctrl+u 清空
                    if k.modifiers.contains(KeyModifiers::CONTROL) && (c == 'u' || c == 'U') {
                        app.search_query.clear();
                    } else if !k.modifiers.contains(KeyModifiers::CONTROL)
                        && !k.modifiers.contains(KeyModifiers::ALT)
                    {
                        app.search_query.push(c);
                    }
                    // 更新搜索列表选中
                    let len = app.filtered_search_ids().len();
                    if len == 0 {
                        app.search_state.select(None);
                    } else {
                        app.search_state.select(Some(0));
                    }
                }
            }
            _ => {}
        },
    }

    false
}

fn current_episode_len(app: &App) -> usize {
    app.detail_show_id
        .and_then(|id| app.show_by_id(id))
        .map(|s| s.episodes.len())
        .unwrap_or(0)
}

fn list_up(state: &mut ListState) {
    let i = match state.selected() {
        Some(i) => i.saturating_sub(1),
        None => 0,
    };
    state.select(Some(i));
}

fn list_down(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let i = match state.selected() {
        Some(i) => (i + 1).min(len - 1),
        None => 0,
    };
    state.select(Some(i));
}

fn table_up(state: &mut ratatui::widgets::TableState) {
    let i = match state.selected() {
        Some(i) => i.saturating_sub(1),
        None => 0,
    };
    state.select(Some(i));
}

fn table_down(state: &mut ratatui::widgets::TableState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let i = match state.selected() {
        Some(i) => (i + 1).min(len - 1),
        None => 0,
    };
    state.select(Some(i));
}

fn next_tab(t: Tab) -> Tab {
    match t {
        Tab::Home => Tab::Library,
        Tab::Library => Tab::Search,
        Tab::Search => Tab::Home,
    }
}
fn prev_tab(t: Tab) -> Tab {
    match t {
        Tab::Home => Tab::Search,
        Tab::Library => Tab::Home,
        Tab::Search => Tab::Library,
    }
}

fn next_filter(f: LibraryFilter) -> LibraryFilter {
    match f {
        LibraryFilter::All => LibraryFilter::LocalOnly,
        LibraryFilter::LocalOnly => LibraryFilter::WantOnly,
        LibraryFilter::WantOnly => LibraryFilter::MissingOnly,
        LibraryFilter::MissingOnly => LibraryFilter::All,
    }
}

/* ----------------------------- UI ----------------------------- */

fn ui(f: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs
            Constraint::Min(1),    // content
            Constraint::Length(2), // status
        ])
        .split(f.area());

    render_tabs(f, root[0], app);

    // 主内容区域：如果 detail_mode == SplitRight && detail_open，则左右分屏
    if app.detail_open && app.detail_mode == DetailMode::SplitRight {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(root[1]);

        render_main_tab(f, chunks[0], app);
        render_detail(f, chunks[1], app, false);
    } else {
        render_main_tab(f, root[1], app);

        // overlay modal
        if app.detail_open && app.detail_mode == DetailMode::OverlayModal {
            let area = centered_rect(86, 84, f.area());
            f.render_widget(Clear, area);
            render_detail(f, area, app, true);
        }
    }

    render_status(f, root[2], app);

    if let Some(toast) = &app.toast {
        // 顶部右侧 toast - 带动画效果
        let w = (toast.text.chars().count() as u16 + 6).min(f.area().width);
        let area = Rect {
            x: f.area().width.saturating_sub(w + 1),
            y: 1,
            width: w,
            height: 3,
        };
        
        // 脉动效果
        let pulse = (app.tick_count % 20) as f32 / 20.0;
        let alpha = (pulse * std::f32::consts::PI).sin();
        
        let style = if alpha > 0.5 {
            Style::default().fg(colors::INFO).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors::TEXT_PRIMARY)
        };
        
        let p = Paragraph::new(Line::from(vec![
            Span::raw("✨ "),
            Span::styled(toast.text.clone(), style),
        ]))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(colors::INFO))
                    .style(Style::default().bg(colors::BG_DARK))
            );
        f.render_widget(Clear, area);
        f.render_widget(p, area);
    }
}

fn render_tabs(f: &mut Frame, area: Rect, app: &App) {
    let tab_names = [
        ("🏠 Home", "主页"),
        ("📚 Library", "媒体库"),
        ("🔍 Search", "搜索"),
    ];
    
    let titles: Vec<Line> = tab_names
        .iter()
        .enumerate()
        .map(|(i, (icon, name))| {
            let is_selected = match app.tab {
                Tab::Home => i == 0,
                Tab::Library => i == 1,
                Tab::Search => i == 2,
            };
            
            let style = if is_selected {
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors::TEXT_DIM)
            };
            
            Line::from(Span::styled(format!("{} {}", icon, name), style))
        })
        .collect();

    let idx = match app.tab {
        Tab::Home => 0,
        Tab::Library => 1,
        Tab::Search => 2,
    };

    let tabs = Tabs::new(titles)
        .select(idx)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    "✦ AniFrz - 动画管理器 ✦",
                    Style::default()
                        .fg(colors::ACCENT)
                        .add_modifier(Modifier::BOLD)
                ))
                .style(Style::default().bg(colors::BG_DARK))
        )
        .highlight_style(
            Style::default()
                .fg(colors::TEXT_HIGHLIGHT)
                .bg(colors::BG_MEDIUM)
        )
        .divider(Span::styled(" │ ", Style::default().fg(colors::BORDER_DIM)));

    f.render_widget(tabs, area);
}

fn render_main_tab(f: &mut Frame, area: Rect, app: &mut App) {
    match app.tab {
        Tab::Home => render_home(f, area, app),
        Tab::Library => render_library(f, area, app),
        Tab::Search => render_search(f, area, app),
    }
}

fn render_home(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .recent
        .iter()
        .enumerate()
        .map(|(idx, r)| {
            let show = app.show_by_id(r.show_id).unwrap();
            let is_selected = app.home_state.selected() == Some(idx);
            
            // 时间标签颜色
            let time_color = match r.when_label {
                "今天" => colors::SUCCESS,
                "昨天" => colors::INFO,
                _ => colors::TEXT_DIM,
            };
            
            let line = Line::from(vec![
                Span::styled("▶ ", Style::default().fg(if is_selected { colors::ACCENT } else { colors::BORDER_DIM })),
                Span::styled(
                    format!("{:<5}", r.when_label),
                    Style::default()
                        .fg(time_color)
                        .add_modifier(Modifier::BOLD)
                ),
                Span::raw(" │ "),
                Span::styled(
                    format!("{}  ", show.title_cn),
                    Style::default().fg(colors::TEXT_PRIMARY)
                ),
                Span::styled(
                    format!("EP{:02}", r.ep_no),
                    Style::default()
                        .fg(colors::SECONDARY)
                        .add_modifier(Modifier::ITALIC)
                ),
                Span::raw("  "),
                Span::styled(
                    format!("⭐ {:.1}", show.rating),
                    Style::default().fg(colors::WARNING)
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    " 📺 近期观看 ",
                    Style::default()
                        .fg(colors::PRIMARY)
                        .add_modifier(Modifier::BOLD)
                ))
                .style(Style::default().bg(colors::BG_DARK))
        )
        .highlight_style(
            Style::default()
                .bg(colors::BG_MEDIUM)
                .fg(colors::TEXT_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        );

    f.render_stateful_widget(list, area, &mut app.home_state);
}

fn render_library(f: &mut Frame, area: Rect, app: &mut App) {
    let ids = app.filtered_library_ids();
    let items: Vec<ListItem> = ids
        .iter()
        .filter_map(|id| app.show_by_id(*id))
        .map(|s| {
            // 使用更美观的图标和配色
            let (local_icon, local_color) = if s.local {
                ("💾", colors::SUCCESS)
            } else {
                ("⊗", colors::TEXT_DIM)
            };
            
            let (want_icon, want_color) = if s.want {
                ("❤️ ", colors::ACCENT)
            } else {
                ("  ", colors::TEXT_DIM)
            };

            let missing_eps = s.episodes.iter().filter(|e| !e.local).count();
            let total_eps = s.episodes.len();
            let progress_pct = if total_eps > 0 {
                ((total_eps - missing_eps) as f32 / total_eps as f32 * 100.0) as u32
            } else {
                0
            };
            
            // 进度条颜色
            let progress_color = if progress_pct == 100 {
                colors::SUCCESS
            } else if progress_pct >= 50 {
                colors::INFO
            } else {
                colors::WARNING
            };
            
            let line = Line::from(vec![
                Span::styled(local_icon, Style::default().fg(local_color)),
                Span::raw(" "),
                Span::styled(want_icon, Style::default().fg(want_color)),
                Span::styled(
                    format!("{}", s.title_cn),
                    Style::default().fg(colors::TEXT_PRIMARY)
                ),
                Span::raw("  "),
                Span::styled(
                    format!("[{}/{}]", total_eps - missing_eps, total_eps),
                    Style::default().fg(progress_color)
                ),
                Span::raw("  "),
                Span::styled(
                    format!("⭐{:.1}", s.rating),
                    Style::default().fg(colors::WARNING)
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let filter_name = match app.lib_filter {
        LibraryFilter::All => "全部",
        LibraryFilter::LocalOnly => "仅本地",
        LibraryFilter::WantOnly => "仅想看",
        LibraryFilter::MissingOnly => "想看但缺失",
    };
    
    let filter_icon = match app.lib_filter {
        LibraryFilter::All => "🌐",
        LibraryFilter::LocalOnly => "💾",
        LibraryFilter::WantOnly => "❤️",
        LibraryFilter::MissingOnly => "⚠️",
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    format!(" 📚 媒体库 {} {} (按 f 切换) ", filter_icon, filter_name),
                    Style::default()
                        .fg(colors::PRIMARY)
                        .add_modifier(Modifier::BOLD)
                ))
                .style(Style::default().bg(colors::BG_DARK))
        )
        .highlight_style(
            Style::default()
                .bg(colors::BG_MEDIUM)
                .fg(colors::TEXT_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        );

    f.render_stateful_widget(list, area, &mut app.library_state);
}

fn render_search(f: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);

    let hint = if app.search_focus {
        "⌨️  输入中：打字 / Backspace / Ctrl+U 清空 / Enter 完成 / Esc 退出"
    } else {
        "💡 按 / 进入输入；上下选结果；回车进详情"
    };
    
    let cursor_char = if app.search_focus {
        if (app.tick_count / 8) % 2 == 0 { "▋" } else { " " }
    } else {
        ""
    };

    let input = Paragraph::new(Line::from(vec![
        Span::styled("🔍 ", Style::default().fg(colors::INFO)),
        Span::styled("搜索: ", Style::default()
            .fg(colors::PRIMARY)
            .add_modifier(Modifier::BOLD)
        ),
        Span::styled(
            format!("{}{}", app.search_query, cursor_char),
            Style::default().fg(if app.search_focus {
                colors::TEXT_HIGHLIGHT
            } else {
                colors::TEXT_PRIMARY
            })
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(if app.search_focus {
                colors::INFO
            } else {
                colors::BORDER
            }))
            .title(Span::styled(
                hint,
                Style::default().fg(colors::TEXT_DIM)
            ))
            .style(Style::default().bg(colors::BG_DARK))
    );

    f.render_widget(input, chunks[0]);

    let ids = app.filtered_search_ids();
    let items: Vec<ListItem> = ids
        .iter()
        .filter_map(|id| app.show_by_id(*id))
        .map(|s| {
            let (local_icon, local_color) = if s.local {
                ("💾", colors::SUCCESS)
            } else {
                ("⊗", colors::ERROR)
            };
            
            let want_icon = if s.want { "❤️ " } else { "  " };
            
            let line = Line::from(vec![
                Span::styled(local_icon, Style::default().fg(local_color)),
                Span::raw(" "),
                Span::styled(want_icon, Style::default().fg(colors::ACCENT)),
                Span::styled(
                    format!("{}", s.title_cn),
                    Style::default().fg(colors::TEXT_PRIMARY).add_modifier(Modifier::BOLD)
                ),
                Span::raw("  "),
                Span::styled(
                    format!("({})", s.title),
                    Style::default().fg(colors::TEXT_DIM).add_modifier(Modifier::ITALIC)
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    format!(" 🎯 搜索结果 ({} 项) ", ids.len()),
                    Style::default()
                        .fg(colors::PRIMARY)
                        .add_modifier(Modifier::BOLD)
                ))
                .style(Style::default().bg(colors::BG_DARK))
        )
        .highlight_style(
            Style::default()
                .bg(colors::BG_MEDIUM)
                .fg(colors::TEXT_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        );

    f.render_stateful_widget(list, chunks[1], &mut app.search_state);
}

fn render_detail(f: &mut Frame, area: Rect, app: &mut App, is_modal: bool) {
    let title = match app.detail_show_id.and_then(|id| app.show_by_id(id)) {
        Some(s) => format!("✦ {} ✦ (按 v 切换呈现 / q 关闭)", s.title_cn),
        None => "详情".to_string(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(colors::ACCENT))
        .title(Span::styled(
            title,
            Style::default()
                .fg(colors::ACCENT)
                .add_modifier(Modifier::BOLD)
        ))
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(colors::BG_DARK));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(show_id) = app.detail_show_id else {
        f.render_widget(
            Paragraph::new("无选择")
                .alignment(Alignment::Center)
                .style(Style::default().fg(colors::TEXT_DIM)),
            inner
        );
        return;
    };
    let Some(show) = app.show_by_id(show_id) else { return; };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(inner);

    // left: info
    let info = render_show_info(show, is_modal, app.tick_count);
    f.render_widget(info, chunks[0]);

    // right: episodes table with beautiful icons
    let rows: Vec<Row> = show
        .episodes
        .iter()
        .enumerate()
        .map(|(idx, ep)| {
            let is_selected = app.detail_ep_state.selected() == Some(idx);
            
            let watched_icon = if ep.watched { "✓" } else { "○" };
            
            let local_icon = if ep.local {
                "💾"
            } else {
                "⊗"
            };
            
            let action = if ep.local {
                ("▶ 播放", colors::PRIMARY)
            } else if ep.downloading {
                // 动画下载进度
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame_idx = (app.tick_count as usize / 3) % frames.len();
                (frames[frame_idx], colors::INFO)
            } else {
                ("⬇ 下载", colors::WARNING)
            };

            let style = if is_selected {
                Style::default().bg(colors::BG_MEDIUM).fg(colors::TEXT_HIGHLIGHT)
            } else {
                Style::default().fg(colors::TEXT_PRIMARY)
            };

            Row::new(vec![
                format!("{:02}", ep.no),
                ep.title.clone(),
                watched_icon.to_string(),
                local_icon.to_string(),
                action.0.to_string(),
            ]).style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(12),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colors::BORDER))
            .title(Span::styled(
                " 📋 分集列表 (↑↓ 选择 | w 标记 | d 下载 | Enter 播放) ",
                Style::default()
                    .fg(colors::SECONDARY)
                    .add_modifier(Modifier::BOLD)
            ))
            .style(Style::default().bg(colors::BG_DARK))
    )
    .header(
        Row::new(vec!["集", "标题", "看", "存", "操作"])
            .style(
                Style::default()
                    .fg(colors::PRIMARY)
                    .add_modifier(Modifier::BOLD)
            )
    )
    .row_highlight_style(
        Style::default()
            .bg(colors::BG_LIGHT)
            .add_modifier(Modifier::BOLD)
    );

    f.render_stateful_widget(table, chunks[1], &mut app.detail_ep_state);
}

fn render_show_info(show: &Show, is_modal: bool, _tick_count: u32) -> Paragraph<'static> {
    let watched_cnt = show.episodes.iter().filter(|e| e.watched).count();
    let local_cnt = show.episodes.iter().filter(|e| e.local).count();
    let downloading_cnt = show.episodes.iter().filter(|e| e.downloading).count();
    let total = show.episodes.len();
    
    // 计算进度百分比
    let progress = if total > 0 {
        (local_cnt as f32 / total as f32 * 100.0) as u32
    } else {
        0
    };

    let mut t = Text::default();
    
    // 标题行
    t.lines.push(Line::from(vec![
        Span::styled("📺 ", Style::default().fg(colors::ACCENT)),
        Span::styled(
            show.title_cn.clone(),
            Style::default()
                .fg(colors::TEXT_HIGHLIGHT)
                .add_modifier(Modifier::BOLD)
        ),
    ]));
    
    t.lines.push(Line::from(
        Span::styled(
            format!("   {}", show.title),
            Style::default()
                .fg(colors::TEXT_DIM)
                .add_modifier(Modifier::ITALIC)
        )
    ));
    
    t.lines.push(Line::from(""));
    
    // 基本信息
    t.lines.push(Line::from(vec![
        Span::styled("📅 年份: ", Style::default().fg(colors::INFO)),
        Span::styled(
            format!("{}", show.year),
            Style::default().fg(colors::TEXT_PRIMARY)
        ),
        Span::raw("   "),
        Span::styled("⭐ 评分: ", Style::default().fg(colors::WARNING)),
        Span::styled(
            format!("{:.1}", show.rating),
            Style::default()
                .fg(colors::WARNING)
                .add_modifier(Modifier::BOLD)
        ),
    ]));
    
    // 标签
    t.lines.push(Line::from(vec![
        Span::styled("🏷️  标签: ", Style::default().fg(colors::SECONDARY)),
        Span::styled(
            show.tags.join(" • "),
            Style::default().fg(colors::TEXT_PRIMARY)
        ),
    ]));
    
    t.lines.push(Line::from(""));
    
    // 状态信息
    let local_icon = if show.local { "✓" } else { "✗" };
    let want_icon = if show.want { "❤️" } else { "🤍" };
    
    t.lines.push(Line::from(vec![
        Span::styled(
            format!("{} 本地", local_icon),
            Style::default().fg(if show.local { colors::SUCCESS } else { colors::TEXT_DIM })
        ),
        Span::raw("   "),
        Span::styled(
            format!("{} 想看", want_icon),
            Style::default().fg(if show.want { colors::ACCENT } else { colors::TEXT_DIM })
        ),
        Span::raw("   "),
        Span::styled(
            format!("📦 {}/{} 集", local_cnt, total),
            Style::default().fg(colors::INFO)
        ),
    ]));
    
    // 进度条
    let bar_width = 30;
    let filled = (progress as usize * bar_width / 100).min(bar_width);
    let empty = bar_width - filled;
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    
    let progress_color = if progress == 100 {
        colors::SUCCESS
    } else if progress >= 50 {
        colors::INFO
    } else {
        colors::WARNING
    };
    
    t.lines.push(Line::from(vec![
        Span::styled("进度 ", Style::default().fg(colors::TEXT_DIM)),
        Span::styled(bar, Style::default().fg(progress_color)),
        Span::styled(
            format!(" {}%", progress),
            Style::default().fg(progress_color).add_modifier(Modifier::BOLD)
        ),
    ]));
    
    t.lines.push(Line::from(vec![
        Span::styled(
            format!("✓ 已看 {} 集", watched_cnt),
            Style::default().fg(colors::SUCCESS)
        ),
        Span::raw("  "),
        Span::styled(
            format!("⬇ 下载中 {} 集", downloading_cnt),
            Style::default().fg(if downloading_cnt > 0 { colors::INFO } else { colors::TEXT_DIM })
        ),
    ]));
    
    t.lines.push(Line::from(""));
    t.lines.push(Line::from(Span::styled(
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
        Style::default().fg(colors::BORDER_DIM)
    )));
    t.lines.push(Line::from(""));

    if is_modal {
        t.lines.push(Line::from(Span::styled(
            "💡 按 v 切换显示模式 | q 关闭",
            Style::default()
                .fg(colors::TEXT_DIM)
                .add_modifier(Modifier::ITALIC)
        )));
        t.lines.push(Line::from(""));
    }

    t.lines.push(Line::from(Span::styled(
        show.summary,
        Style::default().fg(colors::TEXT_PRIMARY)
    )));

    Paragraph::new(t)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(colors::BORDER))
                .title(Span::styled(
                    " ℹ️  作品信息 ",
                    Style::default()
                        .fg(colors::SECONDARY)
                        .add_modifier(Modifier::BOLD)
                ))
                .style(Style::default().bg(colors::BG_DARK))
        )
        .wrap(Wrap { trim: true })
}

fn render_status(f: &mut Frame, area: Rect, app: &App) {
    let (tab_name, tab_color) = match app.tab {
        Tab::Home => ("🏠 Home", colors::PRIMARY),
        Tab::Library => ("📚 Library", colors::SECONDARY),
        Tab::Search => ("🔍 Search", colors::INFO),
    };

    let help = match app.tab {
        Tab::Home => "h/l 或 Tab 切页 │ ↑↓ 选择 │ Enter 详情 │ q 退出",
        Tab::Library => "h/l 或 Tab 切页 │ ↑↓ 选择 │ f 筛选 │ Enter 详情 │ q 退出",
        Tab::Search => "/ 输入 │ ↑↓ 选择 │ Enter 详情/确认 │ Esc 退出输入 │ q 退出",
    };

    let text = Line::from(vec![
        Span::styled(
            format!(" {} ", tab_name),
            Style::default()
                .fg(tab_color)
                .add_modifier(Modifier::BOLD)
        ),
        Span::styled(" │ ", Style::default().fg(colors::BORDER_DIM)),
        Span::styled(help, Style::default().fg(colors::TEXT_DIM)),
    ]);

    let status = Paragraph::new(text)
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(colors::BORDER_DIM))
                .style(Style::default().bg(colors::BG_DARK))
        );

    f.render_widget(status, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/* ----------------------------- Demo Data ----------------------------- */

fn demo_shows() -> Vec<Show> {
    let mut shows = vec![
        Show {
            id: 0,
            title: "Frieren: Beyond Journey's End".into(),
            title_cn: "葬送的芙莉莲".into(),
            year: 2023,
            eps_total: 28,
            rating: 9.2,
            tags: vec!["奇幻", "公路", "治愈"],
            summary: "勇者一行完成旅途后，精灵法师在漫长寿命里回望‘离别’与‘理解’。",
            want: false,
            local: true,
            episodes: make_eps(12, true, 0),
        },
        Show {
            id: 1,
            title: "Bocchi the Rock!".into(),
            title_cn: "孤独摇滚！".into(),
            year: 2022,
            eps_total: 12,
            rating: 8.8,
            tags: vec!["音乐", "日常", "社恐"],
            summary: "社恐少女想组乐队，结果被现实和队友一起狠狠治愈（并社死）。",
            want: true,
            local: false,
            episodes: make_eps(12, false, 0),
        },
        Show {
            id: 2,
            title: "Odd Taxi".into(),
            title_cn: "奇巧计程车".into(),
            year: 2021,
            eps_total: 13,
            rating: 8.7,
            tags: vec!["悬疑", "群像", "都市"],
            summary: "一辆计程车串起多条线索，越看越不对劲，越不对劲越停不下来。",
            want: false,
            local: true,
            episodes: make_eps(13, true, 4), // 缺后 4 集
        },
        Show {
            id: 3,
            title: "Kaguya-sama: Love is War".into(),
            title_cn: "辉夜大小姐想让我告白".into(),
            year: 2019,
            eps_total: 12,
            rating: 8.5,
            tags: vec!["恋爱喜剧", "智斗", "校园"],
            summary: "恋爱像战争，先告白的人就输了——于是他们开始疯狂内耗。",
            want: true,
            local: true, // 部分本地
            episodes: make_eps(12, true, 6), // 缺后 6 集
        },
        Show {
            id: 4,
            title: "Violet Evergarden".into(),
            title_cn: "紫罗兰永恒花园".into(),
            year: 2018,
            eps_total: 13,
            rating: 8.6,
            tags: vec!["治愈", "战后", "书信"],
            summary: "用写信去理解语言与情感，也在理解的过程中慢慢找回自己。",
            want: false,
            local: true,
            episodes: make_eps(13, true, 0),
        },
        Show {
            id: 5,
            title: "Made in Abyss".into(),
            title_cn: "来自深渊".into(),
            year: 2017,
            eps_total: 13,
            rating: 8.9,
            tags: vec!["冒险", "黑童话", "深渊"],
            summary: "可爱画风下是扎心世界观：越往下，越接近真相，也越接近失去。",
            want: true,
            local: false,
            episodes: make_eps(13, false, 0),
        },
    ];

    // 细调一些本地/缺失，让“缺失可下载”的演示更明显
    // show0：本地全有
    // show1：全缺
    // show2：缺后4
    // show3：缺后6
    // show4：全有
    // show5：全缺

    // 给 show3 设置 local=true 但部分集 local=false（make_eps 已做）
    // 给 show2 同理

    // show1/5 local=false, episodes local=false
    for &sid in &[1usize, 5usize] {
        let s = shows.iter_mut().find(|x| x.id == sid).unwrap();
        s.local = false;
        for e in &mut s.episodes {
            e.local = false;
        }
    }

    shows
}

fn make_eps(total: u32, local: bool, missing_tail: u32) -> Vec<Episode> {
    (1..=total)
        .map(|no| {
            let mut ep_local = local;
            if missing_tail > 0 && no > total.saturating_sub(missing_tail) {
                ep_local = false;
            }
            Episode {
                no,
                title: format!("Episode {}", no),
                watched: false,
                local: ep_local,
                downloading: false,
                download_started: None,
            }
        })
        .collect()
}

fn mark_watched(shows: &mut [Show], show_id: usize, range: std::ops::RangeInclusive<u32>) {
    if let Some(s) = shows.iter_mut().find(|x| x.id == show_id) {
        for ep in &mut s.episodes {
            if range.contains(&ep.no) {
                ep.watched = true;
            }
        }
    }
}
