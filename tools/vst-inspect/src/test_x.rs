//! Test X: read the shared global before/after comp.init() to trace the linking mechanism
use std::ffi::c_void;

const K_RESULT_OK: i32 = 0;
const K_NO_INTERFACE: i32 = 0x80004002u32 as i32;
const IID_FUNKNOWN: [u8; 16] = [0,0,0,0,0,0,0,0,0xC0,0,0,0,0,0,0,0x46];
const IID_ICOMPONENT_HANDLER: [u8; 16] = vst3_iid(0x93A0BEA3,0x0BD045DB,0x8E890B0C,0xC1E46AC6);
const IID_ICOMPONENT:         [u8; 16] = vst3_iid(0xE831FF31,0xF2D54301,0x928EBBEE,0x25697802);
const IID_IEDIT_CONTROLLER:   [u8; 16] = vst3_iid(0xDCD7BBE3,0x7742448D,0xA874AACC,0x979C759E);
const IID_IHOST_APPLICATION:  [u8; 16] = vst3_iid(0x58E595CC,0xDB2D4969,0x8B6AAF8C,0x36A664E5);
const IID_IPLUGIN_FACTORY3:   [u8; 16] = vst3_iid(0x4555A2AB,0xC123D4D2,0x94350F8B,0x6A9C4772);

const fn vst3_iid(l1:u32,l2:u32,l3:u32,l4:u32)->[u8;16]{
    [(l1&0xFF)as u8,((l1>>8)&0xFF)as u8,((l1>>16)&0xFF)as u8,((l1>>24)&0xFF)as u8,
     ((l2>>16)&0xFF)as u8,((l2>>24)&0xFF)as u8,(l2&0xFF)as u8,((l2>>8)&0xFF)as u8,
     ((l3>>24)&0xFF)as u8,((l3>>16)&0xFF)as u8,((l3>>8)&0xFF)as u8,(l3&0xFF)as u8,
     ((l4>>24)&0xFF)as u8,((l4>>16)&0xFF)as u8,((l4>>8)&0xFF)as u8,(l4&0xFF)as u8]
}
fn hex16(b:&[u8;16])->String{b.iter().map(|x|format!("{x:02X}")).collect::<Vec<_>>().join("")}
fn cstr(b:&[u8])->String{let e=b.iter().position(|&x|x==0).unwrap_or(b.len());String::from_utf8_lossy(&b[..e]).into_owned()}

unsafe fn vtfn<F:Copy>(obj:*mut c_void,idx:usize)->F{let v=*(obj as *mut*const usize);std::mem::transmute_copy(&*v.add(idx))}
unsafe fn call_release(o:*mut c_void)->u32{let f:unsafe extern "system" fn(*mut c_void)->u32=vtfn(o,2);f(o)}
unsafe fn call_init(o:*mut c_void,ctx:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(o,3);f(o,ctx)}
unsafe fn call_term(o:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void)->i32=vtfn(o,4);f(o)}
unsafe fn ctrl_set_handler(c:*mut c_void,h:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(c,16);f(c,h)}
unsafe fn ctrl_create_view(c:*mut c_void)->*mut c_void{let f:unsafe extern "system" fn(*mut c_void,*const i8)->*mut c_void=vtfn(c,17);f(c,b"editor\0".as_ptr() as*const i8)}
unsafe fn call_qi(o:*mut c_void,iid:&[u8;16],out:*mut*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*const u8,*mut*mut c_void)->i32=vtfn(o,0);f(o,iid.as_ptr(),out)}
unsafe fn qi(o:*mut c_void,iid:&[u8;16])->Option<*mut c_void>{
    let mut out=std::ptr::null_mut();
    if call_qi(o,iid,&mut out)==K_RESULT_OK&&!out.is_null(){Some(out)}else{None}
}
unsafe fn comp_ctrl_cid(comp:*mut c_void)->Option<[u8;16]>{
    let f:unsafe extern "system" fn(*mut c_void,*mut[u8;16])->i32=vtfn(comp,5);
    let mut cid=[0u8;16];if f(comp,&mut cid)==K_RESULT_OK{Some(cid)}else{None}
}

#[repr(C)] struct PClassInfo{cid:[u8;16],_car:i32,category:[u8;32],name:[u8;64]}
unsafe fn fac_count(f:*mut c_void)->i32{let fn_:unsafe extern "system" fn(*mut c_void)->i32=vtfn(f,4);fn_(f)}
unsafe fn fac_class_info(fac:*mut c_void,i:i32,info:*mut PClassInfo)->i32{let f:unsafe extern "system" fn(*mut c_void,i32,*mut PClassInfo)->i32=vtfn(fac,5);f(fac,i,info)}
unsafe fn fac_create(fac:*mut c_void,cid:&[u8;16],iid:&[u8;16])->Option<*mut c_void>{
    let f:unsafe extern "system" fn(*mut c_void,*const u8,*const u8,*mut*mut c_void)->i32=vtfn(fac,6);
    let mut o:*mut c_void=std::ptr::null_mut();
    if f(fac,cid.as_ptr(),iid.as_ptr(),&mut o)==K_RESULT_OK&&!o.is_null(){Some(o)}else{None}
}

// IComponentHandler stub
#[repr(C)] struct CH{v:*const CHVtbl}
#[repr(C)] struct CHVtbl{
    qi:unsafe extern "system" fn(*mut CH,*const u8,*mut*mut c_void)->i32,
    ar:unsafe extern "system" fn(*mut CH)->u32, re:unsafe extern "system" fn(*mut CH)->u32,
    be:unsafe extern "system" fn(*mut CH,u32)->i32, pe:unsafe extern "system" fn(*mut CH,u32,f64)->i32,
    ee:unsafe extern "system" fn(*mut CH,u32)->i32, rc:unsafe extern "system" fn(*mut CH,i32)->i32,
}
unsafe extern "system" fn ch_qi(t:*mut CH,iid:*const u8,o:*mut*mut c_void)->i32{
    let s=&*(iid as*const[u8;16]);
    if s==&IID_ICOMPONENT_HANDLER||s==&IID_FUNKNOWN{*o=t as _;K_RESULT_OK}else{*o=std::ptr::null_mut();K_NO_INTERFACE}
}
unsafe extern "system" fn ch_ar(_:*mut CH)->u32{1}
unsafe extern "system" fn ch_re(_:*mut CH)->u32{1}
unsafe extern "system" fn ch_32(_:*mut CH,_:u32)->i32{K_RESULT_OK}
unsafe extern "system" fn ch_pe(_:*mut CH,_:u32,_:f64)->i32{K_RESULT_OK}
unsafe extern "system" fn ch_rc(_:*mut CH,_:i32)->i32{K_RESULT_OK}
static CHVTBL:CHVtbl=CHVtbl{qi:ch_qi,ar:ch_ar,re:ch_re,be:ch_32,pe:ch_pe,ee:ch_32,rc:ch_rc};

// IHostApplication stub
#[repr(C)] struct HA{v:*const HAVtbl}
#[repr(C)] struct HAVtbl{
    qi:   unsafe extern "system" fn(*mut HA,*const u8,*mut*mut c_void)->i32,
    ar:   unsafe extern "system" fn(*mut HA)->u32,
    re:   unsafe extern "system" fn(*mut HA)->u32,
    name: unsafe extern "system" fn(*mut HA,*mut i16)->i32,
    crei: unsafe extern "system" fn(*mut HA,*const u8,*const u8,*mut*mut c_void)->i32,
}
unsafe extern "system" fn ha_qi(t:*mut HA,iid:*const u8,o:*mut*mut c_void)->i32{
    let s=&*(iid as*const[u8;16]);
    if s==&IID_IHOST_APPLICATION||s==&IID_FUNKNOWN{*o=t as _;K_RESULT_OK}else{*o=std::ptr::null_mut();K_NO_INTERFACE}
}
unsafe extern "system" fn ha_ar(_:*mut HA)->u32{1}
unsafe extern "system" fn ha_re(_:*mut HA)->u32{1}
unsafe extern "system" fn ha_name(_:*mut HA,n:*mut i16)->i32{
    for (i,c) in [b'C',b'a',b'b',b'l',b'e',0].iter().enumerate(){*n.add(i)=*c as i16;}
    K_RESULT_OK
}
unsafe extern "system" fn ha_crei(_:*mut HA,_:*const u8,_:*const u8,o:*mut*mut c_void)->i32{
    *o=std::ptr::null_mut();K_NO_INTERFACE
}
static HAVTBL:HAVtbl=HAVtbl{qi:ha_qi,ar:ha_ar,re:ha_re,name:ha_name,crei:ha_crei};

// Read 8 words from a plugin-owned address (guard with VirtualQuery)
unsafe fn safe_read_words(addr: usize, count: usize) -> Vec<Option<usize>> {
    let mut result = vec![];
    for i in 0..count {
        let ptr = (addr + i * 8) as *const usize;
        // Use VirtualQuery to check if the address is readable
        let mut mbi: windows_sys::Win32::System::Memory::MEMORY_BASIC_INFORMATION =
            std::mem::zeroed();
        let ret = windows_sys::Win32::System::Memory::VirtualQuery(
            ptr as *const c_void,
            &mut mbi,
            std::mem::size_of_val(&mbi),
        );
        if ret == 0 {
            result.push(None);
            continue;
        }
        let readable = mbi.State == windows_sys::Win32::System::Memory::MEM_COMMIT
            && (mbi.Protect & (
                windows_sys::Win32::System::Memory::PAGE_READONLY
                | windows_sys::Win32::System::Memory::PAGE_READWRITE
                | windows_sys::Win32::System::Memory::PAGE_EXECUTE_READ
                | windows_sys::Win32::System::Memory::PAGE_EXECUTE_READWRITE
            )) != 0;
        if readable {
            result.push(Some(*ptr));
        } else {
            result.push(None);
        }
    }
    result
}

fn main(){
    let args:Vec<String>=std::env::args().collect();
    let denoiser=args.get(1).map(|s|s.as_str())
        .unwrap_or(r"C:\Program Files\Common Files\VST3\Bertom Denoiser_x64.vst3");

    println!("=== Test X: shared global value tracing ===\n");

    unsafe{
        let ole32=libloading::Library::new("ole32.dll").unwrap();
        let co_init:libloading::Symbol<unsafe extern "system" fn(*mut c_void,u32)->i32>
            =ole32.get(b"CoInitializeEx\0").unwrap();
        println!("CoInitializeEx(STA) = {:#010x}",co_init(std::ptr::null_mut(),0x2));

        let mut ha_stub=HA{v:&HAVTBL};
        let ha=&mut ha_stub as *mut _ as *mut c_void;
        let mut ch_stub=CH{v:&CHVTBL};
        let h=&mut ch_stub as *mut _ as *mut c_void;

        let lib=libloading::Library::new(denoiser).expect("load");
        let get_factory:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>
            =lib.get(b"GetPluginFactory\0").unwrap();
        let factory=get_factory();
        if let Some(fac3)=qi(factory,&IID_IPLUGIN_FACTORY3){
            let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
            f(fac3,ha);call_release(fac3);
        }
        let mut audio_cid=[0u8;16];
        let mut ctrl_cid=[0u8;16];
        for i in 0..fac_count(factory){
            let mut info=PClassInfo{cid:[0u8;16],_car:0,category:[0u8;32],name:[0u8;64]};
            fac_class_info(factory,i,&mut info);
            let cat=cstr(&info.category);
            if cat.starts_with("Audio Module Class"){audio_cid=info.cid;}
            if cat.contains("Controller"){ctrl_cid=info.cid;}
        }
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            call_init(comp,std::ptr::null_mut());
            if let Some(cid)=comp_ctrl_cid(comp){ctrl_cid=cid;}
            call_term(comp);call_release(comp);
        }
        println!("audio_cid={}  ctrl_cid={}",hex16(&audio_cid),hex16(&ctrl_cid));

        // === Step 1: Create comp, read comp[+0x38] (the shared global address) ===
        println!("\n=== Step 1: read shared global address from comp ===");
        let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT) else {panic!("no comp")};
        // comp is a IComponent pointer. Read comp[+0x38] before init.
        let shared_ptr_addr = *(comp.add(0x38) as *const usize);
        println!("comp[+0x38] (shared global addr) = {shared_ptr_addr:#018x}");

        // Read content of shared global BEFORE comp.init()
        println!("Content at shared global (before comp.init()):");
        let words_before = safe_read_words(shared_ptr_addr, 8);
        for (i, w) in words_before.iter().enumerate() {
            match w {
                Some(v) => println!("  [{:#04x}] = {v:#018x}", i*8),
                None => println!("  [{:#04x}] = <unreadable>", i*8),
            }
        }

        // Init comp
        let cir = call_init(comp, ha);
        println!("\ncomp.init = {cir:#x}  comp_addr={:#018x}", comp as usize);

        // Read content AFTER comp.init()
        println!("Content at shared global (after comp.init()):");
        let words_after = safe_read_words(shared_ptr_addr, 8);
        for (i, w) in words_after.iter().enumerate() {
            match w {
                Some(v) => {
                    let changed = words_before.get(i).and_then(|x|*x) != Some(*v);
                    let marker = if changed { " ← CHANGED!" } else { "" };
                    println!("  [{:#04x}] = {v:#018x}{marker}", i*8);
                }
                None => println!("  [{:#04x}] = <unreadable>", i*8),
            }
        }

        // === Step 2: Create ctrl, read ctrl[+0x20] (should be same address) ===
        println!("\n=== Step 2: create ctrl, verify shared addr ===");
        let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER) else {
            call_term(comp);call_release(comp);panic!("no ctrl")
        };
        let ctrl_shared = *(ctrl.sub(0x30).add(0x20) as *const usize); // ctrl_base[+0x20]
        println!("ctrl_base[+0x20] (shared global addr) = {ctrl_shared:#018x}");
        println!("same as comp? {}",ctrl_shared==shared_ptr_addr);

        // Read content BEFORE ctrl.init()
        println!("Content at shared global (before ctrl.init()):");
        let words_mid = safe_read_words(shared_ptr_addr, 8);
        for (i, w) in words_mid.iter().enumerate() {
            match w {
                Some(v) => {
                    let changed = words_before.get(i).and_then(|x|*x) != Some(*v);
                    let marker = if changed { " ← changed from initial" } else { "" };
                    println!("  [{:#04x}] = {v:#018x}{marker}", i*8);
                }
                None => println!("  [{:#04x}] = <unreadable>", i*8),
            }
        }

        // Init ctrl
        let ctrl_ir = call_init(ctrl, ha);
        let _ = ctrl_set_handler(ctrl, h);
        println!("\nctrl.init = {ctrl_ir:#x}  ctrl_addr={:#018x}", ctrl as usize);

        let field = *(ctrl.add(0x60) as *const usize);
        println!("ctrl[+0x60] (juceCompo) = {field:#018x}");

        // Read content AFTER ctrl.init()
        println!("Content at shared global (after ctrl.init()):");
        let words_final = safe_read_words(shared_ptr_addr, 8);
        for (i, w) in words_final.iter().enumerate() {
            match w {
                Some(v) => {
                    let was = words_before.get(i).and_then(|x|*x).unwrap_or(0);
                    let changed = was != *v;
                    let marker = if changed { " ← CHANGED from initial" } else { "" };
                    println!("  [{:#04x}] = {v:#018x}{marker}", i*8);
                }
                None => println!("  [{:#04x}] = <unreadable>", i*8),
            }
        }

        // Check if comp_addr appears in the shared global content
        let comp_addr = comp as usize;
        println!("\ncomp_addr={comp_addr:#018x}");
        for (i, w) in words_final.iter().enumerate() {
            if *w == Some(comp_addr) {
                println!("  FOUND comp_addr at shared_global[+{:#04x}]!", i*8);
            }
        }

        // === Step 3: Try deep scan — does the shared global point to another level? ===
        println!("\n=== Step 3: second-level dereference of shared global ===");
        if let Some(level1) = words_final.first().and_then(|x|*x) {
            if level1 > 0x1000 && level1 < 0x0000800000000000 {
                println!("shared_global[0] = {level1:#018x}, dereferencing...");
                let words_l2 = safe_read_words(level1, 16);
                for (i, w) in words_l2.iter().enumerate() {
                    match w {
                        Some(v) if *v != 0 => {
                            let is_comp = *v == comp_addr;
                            println!("  [level1+{:#04x}] = {v:#018x}{}",i*8,if is_comp{" ← COMP ADDRESS!"}else{""});
                        }
                        _ => {}
                    }
                }
            }
        }

        // === Step 4: test createView in same thread (STA) without spawning ===
        println!("\n=== Step 4: try createView in STA thread directly (may hang) ===");
        let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
        let params=pc(ctrl);
        println!("params={params}  field={field:#018x}");

        // Attempt createView with a separate thread + 3s timeout
        let ctrl_usize = ctrl as usize;
        let res = std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
        let res2 = res.clone();
        println!("Spawning createView thread...");
        std::thread::spawn(move || {
            let v = ctrl_create_view(ctrl_usize as *mut c_void);
            *res2.lock().unwrap() = Some(!v.is_null());
            if !v.is_null() { call_release(v); }
        });
        std::thread::sleep(std::time::Duration::from_millis(3000));
        let r = *res.lock().unwrap();
        println!("createView = {}",match r{Some(true)=>"SUCCESS",Some(false)=>"null",None=>"TIMEOUT"});

        call_term(ctrl); call_release(ctrl);
        call_term(comp); call_release(comp);
        println!("\nDone.");
    }
}
