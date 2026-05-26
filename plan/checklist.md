# Master Checklist: การพัฒนา Web Browser (Servo Engine + Rust UI) - ฉบับสมบูรณ์

แผนงานและรายการตรวจสอบอย่างละเอียดสำหรับการพัฒนาเว็บเบราว์เซอร์คุณภาพสูง (Production-Grade) ที่รวบรวมแนวคิดแบบ **Brave** (Shields & Privacy), **Arc** (Workspaces & Sidebar), และ **CocCoc** (Media Sniffer & Downloader) เข้าไว้ด้วยกัน

---

## 🛠️ Phase 1: Project Setup & UI Foundation (โครงสร้างระบบและดีไซน์ชีต)
- [x] **1.1 Workspace Setup**
  - [x] คอนฟิก root `Cargo.toml` เปิดใช้ Cargo Workspace
  - [x] สร้าง crate `browser-core` สำหรับเก็บ Engine, Database, Network logic
  - [x] สร้าง crate `browser-ui` สำหรับส่วนติดต่อผู้ใช้ (Slint)
- [x] **1.2 UI Design System (Slint)**
  - [x] ออกแบบ UI โทนสีเข้มพรีเมียมสไตล์ Arc (Charcoal, light grey outlines) ใน [appwindow.slint](file:///Users/weerachit/Documents/foe/browser-ui/ui/appwindow.slint)
  - [x] พัฒนา Component: [tab_bar.slint](file:///Users/weerachit/Documents/foe/browser-ui/ui/tab_bar.slint) (แท็บแนวนอนด้านบนสุด), [common.slint](file:///Users/weerachit/Documents/foe/browser-ui/ui/common.slint) (ไอคอนเวกเตอร์ SVG, ปุ่มมินิมัล)
  - [x] สร้าง Layout หลัก: TabBar ด้านบนสุด, Navbar, และพื้นที่จำลองหน้าเว็บการ์ดลอยโค้งมน (Arc Floating Card)
- [x] **1.3 Rust-UI Event Bindings**
  - [x] เขียนโค้ดเชื่อมต่อสัญญาณ (Callbacks) ระหว่าง UI และ Rust backend ใน `main.rs` และแบ่งกลุ่ม Handlers
  - [x] จัดการเหตุการณ์ Navigate, Add/Close Tab, Toggle Shields และคิวดาวน์โหลด
- [x] **1.4 Compilation & Verification**
  - [x] ตรวจสอบว่าระบบคอมไพล์ผ่านสำเร็จไม่มี Error
  - [x] ยืนยันว่าหน้าต่างเบราว์เซอร์ (App Shell) เปิดขึ้นมาแสดงผลได้ถูกต้องและมีดีไซน์ตรงตามที่กำหนด

---

## 🎨 Phase 2: Rendering Engine & GPU Bridge (การเชื่อมต่อและดึงภาพจาก Servo)
- [ ] **2.1 Graphics Context & OpenGL Texture Sharing**
  - [ ] ตั้งค่า Graphics Context (WGPU, Metal หรือ OpenGL) ร่วมกันระหว่าง Slint และ Servo
  - [ ] สร้าง OpenGL Framebuffer/Texture ใน Servo ให้สามารถเขียนภาพมาเก็บใน GPU Texture ของ Slint
  - [ ] พัฒนาฟังก์ชันการส่งข้อมูล GPU Texture Handle แบบไม่ผ่าน CPU (Zero-copy rendering) เพื่อประสิทธิภาพสูงสุด
- [ ] **2.2 Event Translator (การส่งค่าเมาส์/คีย์บอร์ด)**
  - [ ] ดักจับเหตุการณ์เมาส์ (คลิกซ้าย/ขวา, Scroll, Hover) จาก Slint และแปลงพิกัด (X, Y) ไปยังหน่วยของ Servo
  - [ ] แปลงเหตุการณ์คีย์บอร์ด (Key Press, Key Release, Modifier Keys เช่น Cmd/Ctrl) ส่งต่อไปยัง Servo
  - [ ] พัฒนาระบบ Focus Management เพื่อให้พิมพ์ข้อความในหน้าเว็บได้ทันทีเมื่อคลิกช่อง Input
- [ ] **2.3 Multi-WebView Lifecycle Manager**
  - [ ] สร้างโครงสร้างข้อมูลใน Rust เพื่อควบคุม Instance ของ Servo หลายๆ ตัวพร้อมกัน
  - [ ] พัฒนาระบบ Freeze/Thaw: หยุดการประมวลผลและการใช้พลังงานของแท็บที่อยู่เบื้องหลัง (Background Tabs)
  - [ ] พัฒนาระบบกู้คืน (Crash Recovery): ตรวจจับหากหน้าเว็บแครชและเปลี่ยนเป็นหน้าเตือน (Sad Tab Page)

---

## 🔍 Phase 3: Navigation, Address Bar & Smart Search (การนำทางและระบบค้นหาอัจฉริยะ)
- [ ] **3.1 Smart URL Parser & Search Redirect**
  - [ ] เขียน Rust parser สำหรับวิเคราะห์ Input จาก URL Bar
  - [ ] ตรวจจับประเภท Input: หากขึ้นต้นด้วย `http`/`https` หรือเป็น IP/Domain ให้เปลี่ยนหน้าตรง
  - [ ] หากไม่ตรงรูปแบบ ให้แปลงเป็น Search URL ส่งไปค้นหาต่อบน Search Engine (เช่น Google, DuckDuckGo)
- [ ] **3.2 Autocomplete & Search Suggestions**
  - [ ] พัฒนาฟังก์ชันส่งคำร้องขอเบื้องหลัง (Background request) ไปยัง Google Autocomplete API
  - [ ] แสดงรายการแนะนำพิมพ์ด่วน (Suggestion Dropdown) ใต้ URL Bar ขณะที่ผู้ใช้กำลังพิมพ์
- [ ] **3.3 Navigation Engine & History SQLite**
  - [ ] บันทึกทุกประวัติการเข้าชม (History) ลงในฐานข้อมูล SQLite
  - [ ] พัฒนาระบบ Back-Forward Cache (BFCache) สำหรับเปิดหน้าเดิมย้อนหลังโดยไม่ต้องขอเครือข่ายใหม่

---

## 📂 Phase 4: Arc-Style Window & Workspace Management (การจัดการแท็บขั้นสูง)
- [ ] **4.1 Spaces & Vertical Tab Groups**
  - [ ] พัฒนาโมดูลจัดเก็บกลุ่มแท็บแยกกันตามงาน (เช่น Work, Personal, Shopping)
  - [ ] ออกแบบ UI การจัดระเบียบแท็บแบบลากวาง (Drag and Drop) เพื่อย้ายตำแหน่งแท็บและ Spaces
- [ ] **4.2 Tab Pinning & Automatic Archiving**
  - [ ] ฟีเจอร์พินแท็บ (Pin Tab) เพื่อล็อกแท็บไว้ที่ส่วนบนสุดของแถบข้าง
  - [ ] พัฒนาระบบ Auto-Archive: จัดเก็บแท็บที่ไม่มีการเรียกใช้เกิน 12 ชั่วโมงลงในแท็บที่เก็บถาวร (Archive) เพื่อลดการใช้ Memory
- [ ] **4.3 Command Palette (Cmd+T / Cmd+L)**
  - [ ] ออกแบบกล่องควบคุมด่วนแบบพิมพ์ด่วน (Spotlight Search สไตล์ Arc)
  - [ ] ค้นหาแท็บที่เปิดอยู่, ประวัติเข้าชมย้อนหลัง, บุ๊กมาร์ก, และการตั้งค่าระบบได้จากช่องทางเดียว
- [ ] **4.4 Split View Layout**
  - [ ] พัฒนาฟังก์ชันแบ่งครึ่งหน้าจอ (Split View) ใน Slint ให้เรนเดอร์ Servo WebView สองหน้าต่างคู่กัน
  - [ ] การแชร์ข้อมูล Session หรือ Cookies ร่วมกันระหว่างหน้าจอแยก

---

## 🛡️ Phase 5: Brave-Style Shields & Privacy Engine (ระบบรักษาความปลอดภัยและการบล็อกโฆษณา)
- [ ] **5.1 Adblock Engine (EasyList Parser)**
  - [ ] พัฒนาหรือนำเข้า Crate บล็อกโฆษณา (เช่น `adblock`)
  - [ ] พัฒนาระบบดาวน์โหลดและอัปเดต EasyList และ EasyPrivacy rules อัตโนมัติทุกๆ 24 ชั่วโมง
  - [ ] บันทึกสถิติจำนวนโฆษณาที่ถูกสกัดกั้นลงในระบบ Local Storage เพื่อแสดงผลสะสม
- [ ] **5.2 Network Filter & Request Interceptor**
  - [ ] ตรวจสอบทุกๆ การเชื่อมต่อเครือข่าย (Network Requests) ใน Servo ก่อนส่งจริง
  - [ ] บล็อกสคริปต์โฆษณา, คุกกี้บุคคลที่สาม (Third-party Cookies), และระบบติดตามพฤติกรรม (Trackers)
- [ ] **5.3 HTTPS-Everywhere & Fingerprinting Prevention**
  - [ ] พัฒนาระบบอัปเกรด URL จาก `http://` เป็น `https://` อัตโนมัติ
  - [ ] บล็อกพอร์ตและขัดขวางการตรวจจับลายนิ้วมือบราวเซอร์ (Canvas/Webgl Fingerprinting)
  - [ ] พัฒนาระบบ Cookie Partitioning: แยกคุกกี้ของเว็บโฆษณาไม่ให้ติดตามข้ามเว็บ (Cross-site Tracking)

---

## ⚡ Phase 6: CocCoc-Style Media Grabber & Download Accelerator (ระบบดาวน์โหลดด่วน)
- [ ] **6.1 Media Sniffer (การจับลิงก์วิดีโอ/เพลง)**
  - [ ] พัฒนาระบบวิเคราะห์ HTTP Response Headers (เช่น ค้นหา Content-Type: `video/mp4`, `video/webm`, `application/x-mpegURL`)
  - [ ] ส่งข้อความและลิงก์มีเดียกลับมายัง UI เพื่อแสดงผลแจ้งเตือนว่า "พบวิดีโอพร้อมดาวน์โหลด"
  - [ ] ออกแบบปุ่มและหน้าต่างย่อยสำหรับเลือกขนาดและระดับความชัด (360p, 720p, 1080p, MP3)
- [ ] **6.2 Segmented Multi-threaded Downloader**
  - [ ] ส่งคำร้องขอ HTTP HEAD ไปยังวิดีโอที่จับได้ เพื่อตรวจสอบว่าเซิร์ฟเวอร์รองรับ `Accept-Ranges` หรือไม่
  - [ ] สร้างเธรด (Tokio Tasks) ย่อยๆ (เช่น 8 เธรด) เพื่อแยกดาวน์โหลดส่วนต่างๆ ของไฟล์พร้อมกัน
  - [ ] รวมชิ้นส่วนของไฟล์ทั้งหมดเป็นไฟล์เดียวหลังจากดาวน์โหลดเสร็จสิ้น
- [ ] **6.3 Resumable Downloads & UI Tracker**
  - [ ] บันทึกสถานะการดาวน์โหลดลงใน SQLite เพื่อรองรับการกดหยุดชั่วคราว (Pause) และการดาวน์โหลดต่อหลังเน็ตหลุด (Resume)
  - [ ] ออกแบบ Download Manager Panel บน Sidebar พร้อมแสดง progress bar และความเร็ววินาทีละครั้ง

---

## 💾 Phase 7: Profile & Session Isolation (การแยกบัญชีผู้ใช้งาน)
- [ ] **7.1 Isolated Storage Engine**
  - [ ] สร้างการตั้งค่าโฟลเดอร์สำหรับผู้ใช้แต่ละโปรไฟล์ (เช่น `Work/`, `Personal/`, `Incognito/`)
  - [ ] แยกเก็บฐานข้อมูล SQLite, Cookies, LocalStorage, และ Cache ออกจากกันอย่างสมบูรณ์
- [ ] **7.2 Incognito Mode (โหมดส่วนตัว)**
  - [ ] พัฒนาโหมดส่วนตัวที่ไม่เขียนแคชและคุกกี้ลงดิสก์ถาวร (เก็บเฉพาะใน RAM และทำลายทิ้งทันทีหลังปิดแท็บ)
- [ ] **7.3 Profile Switcher UI**
  - [ ] พัฒนาปุ่มและเมนูการสลับโปรไฟล์แบบรวดเร็วที่ด้านบนของ Sidebar พร้อมภาพแทนตัวผู้ใช้ (Avatar)

---

## 🛠️ Phase 8: Web Developer Tools (เครื่องมือสำหรับนักพัฒนา)
- [ ] **8.1 DOM Tree Inspector**
  - [ ] ดึงข้อมูลโครงสร้างหน้าเว็บ (DOM Tree) จาก Servo ส่งออกมาในรูปแบบ JSON
  - [ ] สร้าง UI ใน Slint เพื่อแสดงโครงสร้าง HTML แบบพับและขยายได้ (Collapsible Tree View)
  - [ ] เชื่อมโยงเมาส์: เมื่อคลิกส่วนประกอบบนหน้าจอเบราว์เซอร์ ให้ไฮไลท์ Element นั้นใน Inspector
- [ ] **8.2 JavaScript Console Bridge**
  - [ ] ดักจับข้อมูล Log จากเครื่องยนต์ JavaScript (Spidermonkey/Servo) (เช่น `console.log`, `console.error`)
  - [ ] แสดงข้อความผิดพลาดในแถบ Console UI พร้อมระบบพิมพ์คำสั่ง JavaScript ส่งกลับไปประมวลผลด่วน
- [ ] **8.3 Network Monitor**
  - [ ] บันทึกและแสดงรายการทรัพยากรที่หน้าเว็บร้องขอ (เช่น ภาพ, สคริปต์, CSS) พร้อมขนาดไฟล์และเวลาที่ใช้โหลด

---

## 🔌 Phase 9: Web Extension System (ระบบส่วนขยายบราวเซอร์)
- [ ] **9.1 Manifest V3 Engine**
  - [ ] เขียนตัวอ่านและตั้งค่าไฟล์ `manifest.json` ของ Extension
  - [ ] กำหนดสิทธิ์และความเข้ากันได้ (Permissions & Content Security Policy)
- [ ] **9.2 Content Script Injector**
  - [ ] พัฒนาระบบแทรกสคริปต์ (Script Injection) ลงในหน้าเว็บเป้าหมายก่อนที่หน้าเว็บจะแสดงผล
- [ ] **9.3 Chrome API Polyfill**
  - [ ] พัฒนา Web API พื้นฐานในบราวเซอร์จำลอง เช่น `chrome.runtime.sendMessage`, `chrome.storage.local`

---

## 🔒 Phase 10: Security, Sandboxing & Resource Management (ความปลอดภัยสูงสุด)
- [ ] **10.1 Multi-Process Architecture**
  - [ ] ปรับสถาปัตยกรรมเป็น Multi-process (Main Process จัดการ UI, Render Process แยกตัวหนึ่งตัวต่อหนึ่งแท็บ)
  - [ ] การเชื่อมต่อสื่อสารระหว่าง Process ด้วย IPC (Inter-Process Communication) ผ่าน Pipes
- [ ] **10.2 Render Process Sandboxing**
  - [ ] เปิดใช้งานการปิดกั้นสิทธิ์ (Sandboxing) สำหรับ Process การเรนเดอร์ ไม่ให้เข้าถึงฮาร์ดดิสก์หรือเครือข่ายโดยไม่ผ่าน Main Process
- [ ] **10.3 Auto-Tab Discarding (ระบบกู้คืน RAM)**
  - [ ] ตรวจวัดปริมาณหน่วยความจำของคอมพิวเตอร์ที่เหลืออยู่
  - [ ] เคลียร์หน่วยความจำของแท็บที่เปิดทิ้งไว้แต่ไม่ได้ใช้งานยาวนาน (Discard Tab) และโหลดใหม่เมื่อกดเลือกอีกครั้ง

---

## ⚡ Phase 11: OS Integration & Performance (การผสานฟังก์ชันเครื่องและการจูน)
- [ ] **11.1 Native Dialogs**
  - [ ] เชื่อมต่อกล่องโต้ตอบการบันทึกไฟล์ (Save File Dialog) และเปิดไฟล์การพิมพ์ของระบบปฏิบัติการ (Print/PDF)
- [ ] **11.2 Audio & Video Integration**
  - [ ] ติดตั้ง Codecs ยอดนิยมร่วมกับ Servo (AAC, H.264, VP9)
  - [ ] ตรวจจับแท็บที่มีการเล่นเสียง (Media tab) และแสดงไอคอนลำโพงเพื่อปิด/เปิดเสียงจาก Sidebar ได้โดยตรง
- [ ] **11.3 Speedometer Benchmark Tuning**
  - [ ] จูนการทำงานของตัวแปรหน่วยความจำและตัวเลือกการคอมไพล์ใน Cargo (เช่น LTO, codegen-units = 1)
  - [ ] แก้ไขจุดคอขวดของการประมวลผลและการเรนเดอร์ใน Rust เพื่อให้ทำคะแนน Benchmark ได้สูง

---

## 📦 Phase 12: Packaging, Tests & Delivery (การทดสอบและปล่อยซอฟต์แวร์)
- [ ] **12.1 Compliance Testing**
  - [ ] รันการตรวจสอบหน้าเว็บกับ W3C Web Platform Tests เพื่อเช็คมาตรฐานการเรนเดอร์ HTML5/CSS3
  - [ ] สร้างการทดสอบจำลองเหตุการณ์ผ่าน GUI (Integration GUI Automation Tests) ด้วยเครื่องมือตรวจสอบ
- [ ] **12.2 CI/CD Build Pipelines**
  - [ ] ตั้งค่า GitHub Actions เพื่อให้บิลด์ไฟล์เบราว์เซอร์อัตโนมัติสำหรับระบบ macOS (.app/.dmg), Windows (.msi), Linux (.deb)
- [ ] **12.3 Auto-Updater**
  - [ ] พัฒนาระบบตรวจเช็คเวอร์ชันใหม่จากโฮสต์เซิร์ฟเวอร์ และดาวน์โหลดแพตช์อัปเดตด่วนเบื้องหลัง
