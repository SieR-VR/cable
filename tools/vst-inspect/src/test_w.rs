//! Test W: dump comp + ctrl layouts, find shared pointers, test new bundle
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
unsafe extern "system" fn ha_crei(_:*mut HA,cid:*const u8,iid:*const u8,o:*mut*mut c_void)->i32{
    let c=&*(cid as*const[u8;16]);
    let i=&*(iid as*const[u8;16]);
    println!("  [HA.createInstance] cid={}  iid={}",hex16(c),hex16(i));
    *o=std::ptr::null_mut();K_NO_INTERFACE
}
static HAVTBL:HAVtbl=HAVtbl{qi:ha_qi,ar:ha_ar,re:ha_re,name:ha_name,crei:ha_crei};

/// Dump object layout: returns Vec of (offset, value) for non-zero words.
unsafe fn dump_object(ptr:*mut c_void, base_offset_bytes:usize, words:usize) -> Vec<(usize, usize)>{
    let base=(ptr as *const usize).sub(base_offset_bytes/8);
    let mut result=vec![];
    for i in 0..words {
        let val=*base.add(i);
        if val!=0 {result.push((i*8, val));}
    }
    result
}

fn main(){
    let args:Vec<String>=std::env::args().collect();
    let denoiser=args.get(1).map(|s|s.as_str())
        .unwrap_or(r"C:\Program Files\Common Files\VST3\Bertom Denoiser_x64.vst3");
    let new_bundle=r"C:\Program Files\Common Files\VST3\Bertom_DenoiserClassic.vst3\Contents\x86_64-win\Bertom_DenoiserClassic.vst3";

    println!("=== Test W: comp+ctrl shared-pointer analysis + new bundle ===\n");

    unsafe{
        let ole32=libloading::Library::new("ole32.dll").unwrap();
        let co_init:libloading::Symbol<unsafe extern "system" fn(*mut c_void,u32)->i32>
            =ole32.get(b"CoInitializeEx\0").unwrap();
        println!("CoInitializeEx(STA) = {:#010x}\n",co_init(std::ptr::null_mut(),0x2));

        let mut ha_stub=HA{v:&HAVTBL};
        let ha=&mut ha_stub as *mut _ as *mut c_void;
        let mut ch_stub=CH{v:&CHVTBL};
        let h=&mut ch_stub as *mut _ as *mut c_void;

        // --- load denoiser ---
        let lib=libloading::Library::new(denoiser).expect("load");
        let get_factory:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>
            =lib.get(b"GetPluginFactory\0").unwrap();
        let factory=get_factory();
        if let Some(fac3)=qi(factory,&IID_IPLUGIN_FACTORY3){
            let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
            let r=f(fac3,ha);println!("fac3.setHostContext={r:#x}");
            call_release(fac3);
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

        // === Test W-1: cross-object shared pointer search ===
        println!("\n=== W-1: cross-object shared pointer search ===");
        let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT) else {panic!("no comp")};
        let cir=call_init(comp,ha);
        let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER) else {panic!("no ctrl")};
        let ctrl_ir=call_init(ctrl,ha);
        let _ =ctrl_set_handler(ctrl,h);

        // comp uses IComponent vtable as base (offset 0 from IComponent pointer)
        let comp_words=dump_object(comp,0,32); // 256 bytes
        let ctrl_words=dump_object(ctrl,0x30,32); // ctrl - 0x30 = ctrl_base; 256 bytes

        println!("comp (IComponent ptr, 256 bytes) non-zero:");
        for (off,val) in &comp_words {println!("  [+{off:#04x}] = {val:#018x}");}
        println!("\nctrl (base = IEditController - 0x30, 256 bytes) non-zero:");
        for (off,val) in &ctrl_words {println!("  [+{off:#04x}] = {val:#018x}");}

        // Find common non-vtable values
        let comp_vals:std::collections::HashSet<usize>=comp_words.iter()
            .filter(|(o,_)|*o>=8) // skip vtable at [0]
            .filter(|(_, v)| *v > 0x1000 && *v < 0x0000700000000000) // likely heap pointers
            .map(|(_,v)|*v).collect();
        let ctrl_vals:std::collections::HashSet<usize>=ctrl_words.iter()
            .filter(|(_, v)| *v > 0x1000 && *v < 0x0000700000000000)
            .map(|(_,v)|*v).collect();
        let shared:Vec<usize>=comp_vals.intersection(&ctrl_vals).copied().collect();
        if shared.is_empty(){
            println!("\nNo shared heap pointers between comp and ctrl objects!");
        } else {
            println!("\nShared heap pointers:");
            for v in &shared {
                let comp_offs:Vec<_>=comp_words.iter().filter(|(_,x)|*x==*v).map(|(o,_)|*o).collect();
                let ctrl_offs:Vec<_>=ctrl_words.iter().filter(|(_,x)|*x==*v).map(|(o,_)|*o).collect();
                println!("  {v:#018x}  comp offsets={comp_offs:?}  ctrl offsets={ctrl_offs:?}");
            }
        }

        // Also look at factory-level pointers in both
        let factory_addr=factory as usize;
        println!("\nfactory_addr={factory_addr:#018x}");
        for (off,val) in &comp_words {
            if *val==factory_addr{println!("  comp[+{off:#04x}] = factory!");}
        }
        for (off,val) in &ctrl_words {
            if *val==factory_addr{println!("  ctrl[+{off:#04x}] = factory!");}
        }

        let field=*(ctrl.add(0x60) as *const usize);
        let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
        println!("\nctrl[+0x60]={field:#018x}  params={}",pc(ctrl));

        call_term(ctrl);call_release(ctrl);
        call_term(comp);call_release(comp);

        // === Test W-2: GetPluginFactory() SECOND call — does it reset state? ===
        println!("\n=== W-2: second GetPluginFactory call ===");
        let factory2=get_factory();
        println!("factory={factory_addr:#018x}  factory2={:#018x}  same={}",factory2 as usize, factory as usize==factory2 as usize);
        // Create comp+ctrl from factory2
        if let (Some(comp2),Some(ctrl2))=(
            fac_create(factory2,&audio_cid,&IID_ICOMPONENT),
            fac_create(factory2,&ctrl_cid,&IID_IEDIT_CONTROLLER)
        ){
            call_init(comp2,ha);
            call_init(ctrl2,ha);
            let _ =ctrl_set_handler(ctrl2,h);
            let f2=*(ctrl2.add(0x60) as *const usize);
            let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl2,8);
            println!("  ctrl[+0x60]={f2:#018x}  params={}",pc(ctrl2));
            call_term(ctrl2);call_release(ctrl2);
            call_term(comp2);call_release(comp2);
        }

        // === Test W-3: new bundle test (if available) ===
        println!("\n=== W-3: new bundle ({new_bundle}) ===");
        match libloading::Library::new(new_bundle){
            Ok(new_lib)=>{
                let gf2:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>
                    =new_lib.get(b"GetPluginFactory\0").unwrap();
                let fac2=gf2();
                if let Some(fac3)=qi(fac2,&IID_IPLUGIN_FACTORY3){
                    let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
                    let r=f(fac3,ha);println!("  fac3.setHostContext={r:#x}");
                    call_release(fac3);
                }
                let mut audio2=[0u8;16];
                let mut ctrl2=[0u8;16];
                for i in 0..fac_count(fac2){
                    let mut info=PClassInfo{cid:[0u8;16],_car:0,category:[0u8;32],name:[0u8;64]};
                    fac_class_info(fac2,i,&mut info);
                    let cat=cstr(&info.category);let name=cstr(&info.name);
                    println!("  [{i}] {:?} {:?}  {}",cat,name,hex16(&info.cid));
                    if cat.starts_with("Audio Module Class"){audio2=info.cid;}
                    if cat.contains("Controller"){ctrl2=info.cid;}
                }
                // Get actual ctrl_cid
                if let Some(c2)=fac_create(fac2,&audio2,&IID_ICOMPONENT){
                    call_init(c2,std::ptr::null_mut());
                    if let Some(cid)=comp_ctrl_cid(c2){ctrl2=cid;}
                    call_term(c2);call_release(c2);
                }
                println!("  audio2={}  ctrl2={}",hex16(&audio2),hex16(&ctrl2));

                if let (Some(c2),Some(ctl2))=(
                    fac_create(fac2,&audio2,&IID_ICOMPONENT),
                    fac_create(fac2,&ctrl2,&IID_IEDIT_CONTROLLER)
                ){
                    call_init(c2,ha);call_init(ctl2,ha);
                    let _ =ctrl_set_handler(ctl2,h);
                    let f=*(ctl2.add(0x60) as *const usize);
                    let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctl2,8);
                    println!("  ctrl[+0x60]={f:#018x}  params={}",pc(ctl2));

                    if f!=0 || pc(ctl2)>0 {
                        // Try createView in spawned thread with 2s timeout
                        let ctrl_usize=ctl2 as usize;
                        let res=std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
                        let res2=res.clone();
                        std::thread::spawn(move||{
                            let v=ctrl_create_view(ctrl_usize as *mut c_void);
                            *res2.lock().unwrap()=Some(!v.is_null());
                            if !v.is_null(){call_release(v);}
                        });
                        std::thread::sleep(std::time::Duration::from_millis(2000));
                        let r=*res.lock().unwrap();
                        println!("  createView={}",match r{Some(true)=>"SUCCESS",Some(false)=>"null",None=>"TIMEOUT"});
                    }

                    call_term(ctl2);call_release(ctl2);
                    call_term(c2);call_release(c2);
                }
                call_release(fac2);
            }
            Err(e)=>println!("  not found: {e}"),
        }

        println!("\nDone.");
    }
}
