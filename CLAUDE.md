# foe — Web Browser (Rust + Slint + wry)

โปรเจคนี้คือ **web browser ที่สร้างด้วย Rust** มีดีไซน์ได้รับแรงบันดาลใจจาก Arc (workspace/sidebar), Brave (Shields/privacy), และ CocCoc (media downloader) ใช้ **Slint** เป็น UI framework และ **wry** เป็น WebView engine

---

## โครงสร้างโปรเจค (Project Structure)

```
foe/
├── Cargo.toml              # Cargo workspace root (resolver = "2")
├── browser_data.db         # SQLite database (history, bookmarks)
├── plan/
│   ├── checklist.md        # 12-phase development checklist (Phase 1 เสร็จแล้ว)
│   └── test_plan.md        # Test cases และ failure mitigation guide
├── browser-core/           # Crate: business logic / backend engine
│   ├── Cargo.toml
│   └── src/lib.rs          # 3 modules: storage, shields, downloader
└── browser-ui/             # Crate: UI application
    ├── Cargo.toml
    ├── build.rs             # slint-build สำหรับคอมไพล์ .slint files
    ├── src/
    │   ├── main.rs          # Entry point, bootstrap services, macOS titlebar config
    │   ├── webview_manager.rs  # wry WebView lifecycle management
    │   └── handlers/
    │       ├── mod.rs       # get_site_type() helper
    │       ├── navigation.rs   # navigate_to, back, forward, reload, shields
    │       ├── tabs.rs         # select_tab, new_tab, close_tab, move_tab
    │       ├── downloader.rs   # trigger_download handler
    │       └── command_bar.rs  # command_bar_submit handler
    └── ui/                  # Slint UI components
        ├── appwindow.slint  # Root window, layout, state properties, callbacks
        ├── tab_bar.slint    # แถบแท็บด้านบนสุด
        ├── navbar.slint     # URL bar, back/forward/reload, shields toggle
        ├── sidebar.slint    # TabInfo / DownloadItem struct definitions
        ├── webview.slint    # Webview placeholder component
        ├── command_bar.slint # Command palette overlay (toggle: F1)
        └── common.slint     # Shared icons, buttons, design tokens
```

---

## Dependencies หลัก

### browser-core
| Crate | Version | หน้าที่ |
|-------|---------|---------|
| `rusqlite` | 0.30 (bundled) | SQLite — history & bookmarks |
| `tokio` | 1.35 (full) | Async runtime |
| `serde` / `serde_json` | 1.0 | Serialization |
| `reqwest` | 0.11 (stream) | HTTP downloads |
| `uuid` | 1.6 (v4) | Download task IDs |
| `url` | 2.5 | URL parsing ใน shields engine |
| `chrono` | 0.4 | Timestamps |

### browser-ui
| Crate | Version | หน้าที่ |
|-------|---------|---------|
| `slint` | 1.6 | UI framework |
| `i-slint-backend-winit` | 1.6 | Winit window backend |
| `wry` | latest | WebView engine (Chromium/WebKit) |
| `tokio` | 1.35 (full) | Async runtime |
| `env_logger` | 0.11 | Logging |

> **หมายเหตุ:** `Cargo.toml` ของ browser-ui ยังมี `servo` dependency อยู่แต่ webview_manager.rs ใช้ `wry` จริง การเชื่อมต่อ Servo engine เป็นส่วนที่ยังไม่ได้ implement (Phase 2)

---

## Architecture Overview

### Threading Model
```
Main Thread (Slint event loop)
    ├── Slint UI rendering
    ├── wry WebView (child window, OS-level)
    └── Tokio async tasks (downloads, DB writes)
```

- **Slint UI** รันบน Main Thread เสมอ — ห้าม block main thread
- **wry WebView** ทำงานเป็น native child window ฝังอยู่ใต้ Slint window
- ใช้ `slint::invoke_from_event_loop()` เพื่อ update UI จาก thread อื่น
- ห้ามใช้ `.lock().unwrap()` แบบ blocking บน main thread

### State Management Pattern
```
AppWindow (Slint) ←→ Rust callbacks (handlers/)
    ↓
WebViewManager (wry) — manages Vec<Option<WebView>>
    ↓
browser-core — Database, ShieldsEngine, DownloadManager
```

- Slint properties เป็น single source of truth สำหรับ UI state
- Rust handlers ใช้ `window.as_weak()` แล้วค่อย `.upgrade()` ใน callback
- `WebViewManager` ใช้ `Rc<RefCell<>>` (single-threaded), ไม่ใช่ `Arc<Mutex<>>`
- Services ที่แชร์ข้าม threads (`Database`, `ShieldsEngine`) ใช้ `Arc<Mutex<>>`

### WebView Layout
- TabBar height: 38px
- Navbar height: 38px
- WebView y-offset: **76px** (`browser-ui/src/webview_manager.rs:139`)
- WebView ปรับ bounds อัตโนมัติเมื่อ window resize ผ่าน winit `WindowEvent::Resized`

---

## Commands

### Build & Run
```bash
# Run browser
cargo run -p browser-ui

# Build release
cargo build -p browser-ui --release

# Check compilation (เร็วกว่า build)
cargo check

# Check specific crate
cargo check -p browser-core
cargo check -p browser-ui
```

### Generate Icons (Node.js script)
```bash
cd browser-ui
npm install
node scripts/generate-icons.mjs
```

---

## Design System (Slint)

### Color Palette
| Token | Value | Usage |
|-------|-------|-------|
| Background | `#1E1F22` | Main window background |
| Surface | `#2C2D30` | Cards, panels |
| Border | `#3A3B3E` | Dividers, outlines |
| Text Primary | `#FFFFFF` | Main text |
| Text Secondary | `#71717A` | Muted labels |
| Accent | `#18181B` | Buttons, progress bars |
| Download Panel BG | `#FFFFFFD0` | Floating download widget |

### Window Config (macOS)
- `fullsize_content_view: true` — content extends under titlebar
- `title_hidden: true` + `titlebar_transparent: true` — custom titlebar area
- `has_titlebar_spacing` property ส่งลงไปยัง TabBar/Navbar เพื่อ offset ให้ถูกต้อง
- Default window size: **900 × 620** logical pixels

### Keyboard Shortcuts
| Key | Action |
|-----|--------|
| `F1` | Toggle Command Bar |

---

## browser-core Modules

### `storage::Database` (`browser-core/src/lib.rs:1`)
- SQLite database เก็บที่ `browser_data.db` (root directory)
- Tables: `history` (id, url, title, visit_time) และ `bookmarks` (id, url, title, added_time)
- `add_history_entry(url, title)` — บันทึก timestamp UTC อัตโนมัติ
- `get_history(limit)` — ดึงประวัติเรียงจากใหม่ไปเก่า

### `shields::ShieldsEngine` (`browser-core/src/lib.rs:88`)
- Host-based domain blocking (stub blocklist)
- Blocked domains เริ่มต้น: `doubleclick.net`, `google-analytics.com`, `ads.youtube.com`, `adservice.google.com`
- `should_block(url)` — parse URL แล้วเช็ค host ว่าตรงกับ blocklist
- Toggle ผ่าน `set_enabled(bool)`

### `downloader::DownloadManager` (`browser-core/src/lib.rs:142`)
- จัดการ `DownloadTask` struct: `{ id, url, filename, total_size, downloaded_size, status }`
- Status values: `"Pending"`, `"Downloading"`, `"Completed"`, `"Failed"`
- เก็บ tasks ใน `Arc<Mutex<Vec<DownloadTask>>>` รองรับ multi-threaded access
- ยังไม่มี actual download logic — Phase 6

---

## UI Handlers

### `handlers::navigation` (`browser-ui/src/handlers/navigation.rs`)
- `clean_url_input()` — แปลง input เป็น URL ที่ถูกต้อง:
  - มี protocol → ใช้ตรงๆ
  - มี `.` และไม่มีช่องว่าง → เติม `https://`
  - อื่นๆ → Google Search URL (URL-encoded)
- `on_navigate_to` → เช็ค shields → บันทึก history → load ใน webview
- `on_toggle_shields` → toggle `ShieldsEngine` และ update `shields_active` property

### `handlers::tabs` (`browser-ui/src/handlers/tabs.rs`)
- แต่ละ tab ใน `Vec<TabInfo>` ตรงกับ `WebView` ใน `WebViewManager.webviews[i]`
- `close_tab` — ป้องกันปิดแท็บสุดท้าย (tabs.len() <= 1 → return)
- `move_tab(from, to)` — ย้าย TabInfo array และ webviews array พร้อมกัน

### `webview_manager::WebViewManager`
- สร้าง WebView ด้วย `WebViewBuilder::new().build_as_child(winit_window)`
- `navigation_handler` และ `document_title_changed_handler` ใช้ `slint::invoke_from_event_loop()` เพื่อ update UI
- `update_bounds_for_active()` — คำนวณ bounds จาก physical size → logical size โดยหาร `scale_factor`

---

## Development Phases

สถานะปัจจุบัน: **Phase 1 เสร็จแล้ว** (Project Setup & UI Foundation)

| Phase | หัวข้อ | สถานะ |
|-------|--------|--------|
| 1 | Project Setup & UI Foundation | ✅ เสร็จ |
| 2 | Rendering Engine & GPU Bridge | ⬜ รอ |
| 3 | Navigation, Address Bar & Smart Search | ⬜ รอ |
| 4 | Arc-Style Window & Workspace Management | ⬜ รอ |
| 5 | Brave-Style Shields & Privacy Engine | ⬜ รอ |
| 6 | CocCoc-Style Media Grabber & Download Accelerator | ⬜ รอ |
| 7 | Profile & Session Isolation | ⬜ รอ |
| 8 | Web Developer Tools | ⬜ รอ |
| 9 | Web Extension System (Manifest V3) | ⬜ รอ |
| 10 | Security, Sandboxing & Resource Management | ⬜ รอ |
| 11 | OS Integration & Performance | ⬜ รอ |
| 12 | Packaging, Tests & Delivery | ⬜ รอ |

รายละเอียดแต่ละ phase ดูได้ที่ `plan/checklist.md`

---

## Critical Technical Risks

### 1. Thread Deadlock (UI freeze)
- Slint รันบน Main Thread, Tokio tasks รันบน background threads
- **ห้ามใช้** synchronous lock (`.lock().unwrap()`) เป็นเวลานานบน main thread
- **ให้ใช้** `slint::invoke_from_event_loop()` สำหรับ UI updates จาก async tasks

### 2. WebView Bounds Sync
- wry WebView เป็น native OS window วางซ้อนบน Slint
- ต้องเรียก `update_bounds_for_active()` ทุกครั้งที่ window resize
- y-offset คือ `76px` (TabBar 38px + Navbar 38px) — ถ้าเปลี่ยนขนาด component ต้องแก้ค่านี้ด้วย

### 3. Slint Callback Lifetime
- ทุก callback ใน Slint ต้องใช้ `window.as_weak()` แล้ว `.upgrade()` ภายใน closure
- อย่า capture `window` โดยตรงใน closure — จะทำให้เกิด reference cycle

---

## Patterns & Conventions

### URL Input Normalization
ดู `clean_url_input()` ใน `navigation.rs:23` — logic canonical สำหรับแปลง user input เป็น URL

### Slint Model Updates
```rust
// Pattern มาตรฐานสำหรับ update tabs list
let tabs_model = window.get_tabs();
let mut tabs: Vec<TabInfo> = tabs_model.iter().collect();
// ... แก้ไข tabs ...
window.set_tabs(ModelRc::new(VecModel::from(tabs)));
```

### Adding a New Handler
1. สร้างไฟล์ใหม่ใน `browser-ui/src/handlers/`
2. declare `pub mod <name>;` ใน `handlers/mod.rs`
3. เรียก `handlers::<name>::setup(&window, ...)` ใน `main.rs`
4. เพิ่ม callback ใน `appwindow.slint` ถ้าต้องการ UI trigger

### Database Path
`browser_data.db` อยู่ที่ **current working directory** ตอน run — ปกติจะเป็น root ของ workspace (`foe/`)
