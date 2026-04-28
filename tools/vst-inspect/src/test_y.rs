//! Test Y: force ctrl[+0x60] = comp and test if createView succeeds,
//! then try various offsets around comp to see which makes createView work.
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

// Try createView with a 3-second timeout.
// Returns: Some(true) = returned non-null IPlugView, Some(false) = returned null, None = timeout
unsafe fn try_create_view(ctrl: *mut c_void) -> Option<bool> {
    let ctrl_usize = ctrl as usize;
    let res = std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
    let res2 = res.clone();
    std::thread::spawn(move || {
        let v = ctrl_create_view(ctrl_usize as *mut c_void);
        *res2.lock().unwrap() = Some(!v.is_null());
        if !v.is_null() { call_release(v); }
    });
    std::thread::sleep(std::time::Duration::from_millis(3000));
    let result = *res.lock().unwrap();
    result
}

fn main(){
    let args:Vec<String>=std::env::args().collect();
    let denoiser=args.get(1).map(|s|s.as_str())
        .unwrap_or(r"C:\Program Files\Common Files\VST3\Bertom Denoiser_x64.vst3");

    println!("=== Test Y: force ctrl[+0x60] = comp, test createView ===\n");

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
            f(fac3,ha); call_release(fac3);
        }
        let mut audio_cid=[0u8;16]; let mut ctrl_cid=[0u8;16];
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

        // ─── Attempt 1: QI for IEditController from comp (single component check) ─
        println!("\n─── Attempt 1: QI(comp, IEditController) ───");
        let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT) else {panic!()};
        call_init(comp, ha);
        let ctrl_from_qi = qi(comp, &IID_IEDIT_CONTROLLER);
        println!("QI(comp, IEditController) = {}",
            ctrl_from_qi.map(|p|format!("{:#018x}", p as usize)).unwrap_or("null".into()));
        if let Some(ctrl_q) = ctrl_from_qi {
            let _ = ctrl_set_handler(ctrl_q, h);
            let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl_q,8);
            println!("  params={}", pc(ctrl_q));
            println!("  createView (3s)...");
            let r = try_create_view(ctrl_q);
            println!("  createView = {}",match r{Some(true)=>"SUCCESS",Some(false)=>"null",None=>"TIMEOUT"});
            call_release(ctrl_q);
        }
        call_term(comp); call_release(comp);

        // ─── Attempt 2: standard init, force ctrl[+0x60] = comp, createView ────
        println!("\n─── Attempt 2: force ctrl[+0x60] = comp ───");
        let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT) else {panic!()};
        let comp_addr = comp as usize;
        call_init(comp, ha);
        let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER) else {
            call_term(comp);call_release(comp);panic!()
        };
        call_init(ctrl, ha);
        let _ = ctrl_set_handler(ctrl, h);

        let juceCompo_before = *(ctrl.add(0x60) as *const usize);
        println!("ctrl[+0x60] before force = {juceCompo_before:#018x}");

        // Force it: ctrl is IEditController, so ctrl[+0x60] = base+0x90... but
        // the createView does `mov rdi, [rbp+0x90]` where rbp = ctrl_ptr - 0x30.
        // So [rbp+0x90] = (ctrl-0x30)[+0x90] = ctrl[+0x60]. Same thing.
        *(ctrl.add(0x60) as *mut usize) = comp_addr;
        let juceCompo_after = *(ctrl.add(0x60) as *const usize);
        println!("ctrl[+0x60] after  force = {juceCompo_after:#018x}");
        println!("comp_addr             =  {comp_addr:#018x}");

        let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
        println!("params={}", pc(ctrl));
        println!("createView (3s)...");
        let r = try_create_view(ctrl);
        println!("createView = {}",match r{Some(true)=>"SUCCESS",Some(false)=>"null",None=>"TIMEOUT"});

        // ─── Attempt 3: force different offsets around comp ─────────────────────
        println!("\n─── Attempt 3: try offsets 0x00..0x50 into comp ───");
        for delta in [0usize, 8, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40, 0x50] {
            let candidate = comp_addr + delta;
            // fresh init cycle for each attempt
            let Some(comp2)=fac_create(factory,&audio_cid,&IID_ICOMPONENT) else {break};
            let comp2_addr = comp2 as usize;
            call_init(comp2, ha);
            let Some(ctrl2)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER) else {
                call_term(comp2);call_release(comp2);break
            };
            call_init(ctrl2, ha);
            let _ = ctrl_set_handler(ctrl2, h);

            // force ctrl[+0x60] = comp2 + delta
            *(ctrl2.add(0x60) as *mut usize) = comp2_addr + delta;

            let ctrl2_usize = ctrl2 as usize;
            let res = std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
            let res2 = res.clone();
            std::thread::spawn(move || {
                let v = ctrl_create_view(ctrl2_usize as *mut c_void);
                *res2.lock().unwrap() = Some(!v.is_null());
                if !v.is_null() { call_release(v); }
            });
            std::thread::sleep(std::time::Duration::from_millis(2000));
            let rv = *res.lock().unwrap();
            println!("  delta=+{delta:#04x} (comp+{delta:#04x}): {}",
                match rv{Some(true)=>"SUCCESS",Some(false)=>"null",None=>"TIMEOUT"});

            call_term(ctrl2); call_release(ctrl2);
            call_term(comp2); call_release(comp2);

            if rv == Some(true) {
                println!("  *** WORKING OFFSET FOUND: comp[+{delta:#04x}] ***");
                break;
            }
        }

        // cleanup attempt 2
        call_term(ctrl); call_release(ctrl);
        call_term(comp); call_release(comp);

        println!("\nDone.");
    }
}
