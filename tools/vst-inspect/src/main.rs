//! VST3 Inspector v6 - IPluginFactory3::setHostContext test
use std::ffi::c_void;
use std::sync::atomic::{AtomicI64, Ordering};

const K_RESULT_OK: i32 = 0;
const K_NO_INTERFACE: i32 = 0x80004002u32 as i32;
const IID_FUNKNOWN: [u8; 16] = [0,0,0,0,0,0,0,0,0xC0,0,0,0,0,0,0,0x46];
const IID_ICOMPONENT_HANDLER: [u8; 16] = vst3_iid(0x93A0BEA3,0x0BD045DB,0x8E890B0C,0xC1E46AC6);
const IID_IBSTREAM:           [u8; 16] = vst3_iid(0xC3BF6EA2,0x3099496A,0x84FB755C,0x90775381);
const IID_ICOMPONENT:         [u8; 16] = vst3_iid(0xE831FF31,0xF2D54301,0x928EBBEE,0x25697802);
const IID_IEDIT_CONTROLLER:   [u8; 16] = vst3_iid(0xDCD7BBE3,0x7742448D,0xA874AACC,0x979C759E);
const IID_ICONNECTION_POINT:  [u8; 16] = vst3_iid(0x70A4156F,0x6E6E5260,0xACB9BF57,0x7938A98C);
const IID_IHOST_APPLICATION:  [u8; 16] = vst3_iid(0x58E595CC,0xDB2D4969,0x8B6AAF8C,0x36A664E5);
// IPluginFactory3: factory-level setHostContext
const IID_IPLUGIN_FACTORY3:   [u8; 16] = vst3_iid(0x4555A2AB,0xC123D4D2,0x94350F8B,0x6A9C4772);

const fn vst3_iid(l1:u32,l2:u32,l3:u32,l4:u32)->[u8;16]{
    [(l1&0xFF)as u8,((l1>>8)&0xFF)as u8,((l1>>16)&0xFF)as u8,((l1>>24)&0xFF)as u8,
     ((l2>>16)&0xFF)as u8,((l2>>24)&0xFF)as u8,(l2&0xFF)as u8,((l2>>8)&0xFF)as u8,
     ((l3>>24)&0xFF)as u8,((l3>>16)&0xFF)as u8,((l3>>8)&0xFF)as u8,(l3&0xFF)as u8,
     ((l4>>24)&0xFF)as u8,((l4>>16)&0xFF)as u8,((l4>>8)&0xFF)as u8,(l4&0xFF)as u8]
}
fn hex16(b:&[u8;16])->String{b.iter().map(|x|format!("{x:02X}")).collect::<Vec<_>>().join("")}
fn cstr(b:&[u8])->String{let e=b.iter().position(|&x|x==0).unwrap_or(b.len());String::from_utf8_lossy(&b[..e]).into_owned()}

// raw vtable helpers
unsafe fn vtfn<F:Copy>(obj:*mut c_void,idx:usize)->F{let v=*(obj as *mut*const usize);std::mem::transmute_copy(&*v.add(idx))}
unsafe fn call_qi(o:*mut c_void,iid:&[u8;16],out:*mut*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*const u8,*mut*mut c_void)->i32=vtfn(o,0);f(o,iid.as_ptr(),out)}
unsafe fn call_release(o:*mut c_void)->u32{let f:unsafe extern "system" fn(*mut c_void)->u32=vtfn(o,2);f(o)}
unsafe fn call_init(o:*mut c_void,ctx:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(o,3);f(o,ctx)}
unsafe fn call_term(o:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void)->i32=vtfn(o,4);f(o)}
// IEditController
unsafe fn ctrl_set_handler(c:*mut c_void,h:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(c,16);f(c,h)}
unsafe fn ctrl_create_view(c:*mut c_void)->*mut c_void{let f:unsafe extern "system" fn(*mut c_void,*const i8)->*mut c_void=vtfn(c,17);f(c,b"editor\0".as_ptr() as*const i8)}
// IConnectionPoint: [3]=connect [4]=disconnect
unsafe fn cp_connect(a:*mut c_void,b_cp:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(a,3);f(a,b_cp)}
// IComponent: [5]=getControllerClassId
unsafe fn comp_ctrl_cid(comp:*mut c_void)->Option<[u8;16]>{
    let f:unsafe extern "system" fn(*mut c_void,*mut[u8;16])->i32=vtfn(comp,5);
    let mut cid=[0u8;16];if f(comp,&mut cid)==K_RESULT_OK{Some(cid)}else{None}
}
// Factory: [4]=count [5]=getClassInfo [6]=createInstance
#[repr(C)] struct PClassInfo{cid:[u8;16],_car:i32,category:[u8;32],name:[u8;64]}
unsafe fn fac_count(f:*mut c_void)->i32{let fn_:unsafe extern "system" fn(*mut c_void)->i32=vtfn(f,4);fn_(f)}
unsafe fn fac_class_info(fac:*mut c_void,i:i32,info:*mut PClassInfo)->i32{let f:unsafe extern "system" fn(*mut c_void,i32,*mut PClassInfo)->i32=vtfn(fac,5);f(fac,i,info)}
unsafe fn fac_create(fac:*mut c_void,cid:&[u8;16],iid:&[u8;16])->Option<*mut c_void>{
    let f:unsafe extern "system" fn(*mut c_void,*const u8,*const u8,*mut*mut c_void)->i32=vtfn(fac,6);
    let mut o:*mut c_void=std::ptr::null_mut();
    if f(fac,cid.as_ptr(),iid.as_ptr(),&mut o)==K_RESULT_OK&&!o.is_null(){Some(o)}else{None}
}
unsafe fn qi(o:*mut c_void,iid:&[u8;16])->Option<*mut c_void>{
    let mut out=std::ptr::null_mut();
    if call_qi(o,iid,&mut out)==K_RESULT_OK&&!out.is_null(){Some(out)}else{None}
}

// ---- IComponentHandler stub ----
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

// ---- IBStream stub (empty) ----
#[repr(C)] struct BS{v:*const BSVtbl,pos:AtomicI64,data:Vec<u8>}
#[repr(C)] struct BSVtbl{
    qi:unsafe extern "system" fn(*mut BS,*const u8,*mut*mut c_void)->i32,
    ar:unsafe extern "system" fn(*mut BS)->u32, re:unsafe extern "system" fn(*mut BS)->u32,
    read:unsafe extern "system" fn(*mut BS,*mut c_void,i32,*mut i32)->i32,
    write:unsafe extern "system" fn(*mut BS,*mut c_void,i32,*mut i32)->i32,
    seek:unsafe extern "system" fn(*mut BS,i64,i32,*mut i64)->i32,
    tell:unsafe extern "system" fn(*mut BS,*mut i64)->i32,
}
unsafe extern "system" fn bs_qi(t:*mut BS,iid:*const u8,o:*mut*mut c_void)->i32{
    let s=&*(iid as*const[u8;16]);
    if s==&IID_IBSTREAM||s==&IID_FUNKNOWN{*o=t as _;K_RESULT_OK}else{*o=std::ptr::null_mut();K_NO_INTERFACE}
}
unsafe extern "system" fn bs_ar(_:*mut BS)->u32{1}
unsafe extern "system" fn bs_re(_:*mut BS)->u32{1}
unsafe extern "system" fn bs_read(t:*mut BS,buf:*mut c_void,n:i32,rd:*mut i32)->i32{
    let s=&*t;let p=s.pos.load(Ordering::SeqCst)as usize;
    let av=s.data.len().saturating_sub(p).min(n.max(0)as usize);
    if av>0{std::ptr::copy_nonoverlapping(s.data.as_ptr().add(p),buf as*mut u8,av);}
    if !rd.is_null(){*rd=av as i32;}s.pos.store((p+av)as i64,Ordering::SeqCst);K_RESULT_OK
}
unsafe extern "system" fn bs_write(t:*mut BS,buf:*mut c_void,n:i32,wr:*mut i32)->i32{
    if n<=0{if !wr.is_null(){*wr=0;}return K_RESULT_OK;}
    let s=&mut *t;
    let bytes=std::slice::from_raw_parts(buf as*const u8,n as usize);
    let pos=s.pos.load(Ordering::SeqCst)as usize;
    if pos+bytes.len()>s.data.len(){s.data.resize(pos+bytes.len(),0);}
    s.data[pos..pos+bytes.len()].copy_from_slice(bytes);
    s.pos.store((pos+bytes.len())as i64,Ordering::SeqCst);
    if !wr.is_null(){*wr=n;}
    K_RESULT_OK
}
unsafe extern "system" fn bs_seek(t:*mut BS,pos:i64,mode:i32,res:*mut i64)->i32{
    let s=&*t;let l=s.data.len()as i64;
    let np=match mode{0=>pos,1=>s.pos.load(Ordering::SeqCst)+pos,2=>l+pos,_=>return 1};
    s.pos.store(np.max(0),Ordering::SeqCst);if !res.is_null(){*res=s.pos.load(Ordering::SeqCst);}K_RESULT_OK
}
unsafe extern "system" fn bs_tell(t:*mut BS,pos:*mut i64)->i32{if !pos.is_null(){*pos=(*t).pos.load(Ordering::SeqCst);}K_RESULT_OK}
static BSVTBL:BSVtbl=BSVtbl{qi:bs_qi,ar:bs_ar,re:bs_re,read:bs_read,write:bs_write,seek:bs_seek,tell:bs_tell};
fn mk_stream()->BS{BS{v:&BSVTBL,pos:AtomicI64::new(0),data:vec![]}}

// ---- IHostApplication stub ----
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
    let iid_str = s.iter().map(|x|format!("{x:02X}")).collect::<Vec<_>>().join("");
    let known = if s==&IID_IHOST_APPLICATION{"IHostApplication"}else if s==&IID_FUNKNOWN{"FUnknown"}else{"unknown"};
    println!("  [HA.qi] asked for {iid_str} ({known})");
    if s==&IID_IHOST_APPLICATION||s==&IID_FUNKNOWN{*o=t as _;K_RESULT_OK}else{*o=std::ptr::null_mut();K_NO_INTERFACE}
}
unsafe extern "system" fn ha_ar(_:*mut HA)->u32{1}
unsafe extern "system" fn ha_re(_:*mut HA)->u32{1}
unsafe extern "system" fn ha_name(_:*mut HA,n:*mut i16)->i32{
    // "Cable\0" as UTF-16 LE
    for (i,c) in [b'C',b'a',b'b',b'l',b'e',0].iter().enumerate(){*n.add(i)=*c as i16;}
    K_RESULT_OK
}
unsafe extern "system" fn ha_crei(_:*mut HA,cid:*const u8,iid:*const u8,o:*mut*mut c_void)->i32{
    let c=&*(cid as*const[u8;16]);
    let i=&*(iid as*const[u8;16]);
    let cs=c.iter().map(|x|format!("{x:02X}")).collect::<Vec<_>>().join("");
    let is=i.iter().map(|x|format!("{x:02X}")).collect::<Vec<_>>().join("");
    println!("  [HA.createInstance] cid={cs}  iid={is}");
    *o=std::ptr::null_mut();K_NO_INTERFACE
}
static HAVTBL:HAVtbl=HAVtbl{qi:ha_qi,ar:ha_ar,re:ha_re,name:ha_name,crei:ha_crei};

fn main(){
    let args:Vec<String>=std::env::args().collect();
    let path=args.get(1).map(|s|s.as_str()).unwrap_or(r"C:\Program Files\Common Files\VST3\Bertom Denoiser_x64.vst3");
    println!("=== VST3 Inspector v6: {path} ===\n");
    unsafe{
        let ole32=libloading::Library::new("ole32.dll").unwrap();
        let co_init:libloading::Symbol<unsafe extern "system" fn(*mut c_void,u32)->i32>=ole32.get(b"CoInitializeEx\0").unwrap();
        println!("CoInitializeEx(STA) -> {:#010x}\n",co_init(std::ptr::null_mut(),0x2));

        let lib=libloading::Library::new(path).expect("load");
        let get_factory:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>=lib.get(b"GetPluginFactory\0").expect("sym");
        let factory=get_factory();

        // --- IPluginFactory3 setHostContext probe ---
        let mut ha_stub=HA{v:&HAVTBL};
        let ha=&mut ha_stub as *mut _ as *mut c_void;

        let has_fac3 = qi(factory, &IID_IPLUGIN_FACTORY3);
        if let Some(fac3)=has_fac3{
            // setHostContext is vtable[9] on IPluginFactory3
            // (IPluginFactory3 extends IPluginFactory2 extends IPluginFactory)
            // IPluginFactory:  [0]qi [1]ar [2]re [3]getFactoryInfo [4]countClasses [5]getClassInfo [6]createInstance
            // IPluginFactory2: [7]getClassInfo2
            // IPluginFactory3: [8]getClassInfoUnicode [9]setHostContext
            let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
            let r=f(fac3,ha);
            println!("IPluginFactory3::setHostContext(HA) = {r:#x}");
            call_release(fac3);
        } else {
            println!("factory does NOT implement IPluginFactory3 (no setHostContext)");
        }
        println!();

        let mut audio_cid=[0u8;16];
        let mut ctrl_cid=[0u8;16];
        for i in 0..fac_count(factory){
            let mut info=PClassInfo{cid:[0u8;16],_car:0,category:[0u8;32],name:[0u8;64]};
            fac_class_info(factory,i,&mut info);
            let cat=cstr(&info.category);let name=cstr(&info.name);
            println!("  [{i}] {cat:?} {name:?}  {}",hex16(&info.cid));
            if cat.starts_with("Audio Module Class"){audio_cid=info.cid;}
            if cat.contains("Controller"){ctrl_cid=info.cid;}
        }
        // Get ctrl_cid from IComponent
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            call_init(comp,std::ptr::null_mut());
            if let Some(cid)=comp_ctrl_cid(comp){ctrl_cid=cid;}
            call_term(comp);call_release(comp);
        }
        println!("\naudio_cid = {}",hex16(&audio_cid));
        println!("ctrl_cid  = {}\n",hex16(&ctrl_cid));

        let mut ch_stub=CH{v:&CHVTBL};
        let h=&mut ch_stub as *mut _ as *mut c_void;

        // Previous tests A-E (without factory setHostContext)
        test_createview("A: null-ctx  no-handler          ", factory, &ctrl_cid, h, false, false, std::ptr::null_mut());
        test_createview("B: host-ctx  no-handler          ", factory, &ctrl_cid, h, false, false, ha);
        test_createview("C: null-ctx  +handler            ", factory, &ctrl_cid, h, true, false, std::ptr::null_mut());
        test_createview("D: host-ctx  +handler            ", factory, &ctrl_cid, h, true, false, ha);

        // Test F: factory-level setHostContext THEN create controller with host context + handler
        println!("\n--- After factory.setHostContext ---");
        if let Some(fac3)=qi(factory,&IID_IPLUGIN_FACTORY3){
            let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
            let r=f(fac3,ha);
            print!("setHostContext={r:#x}  ");
            call_release(fac3);
        }
        test_createview("F: fac3-ctx  host-ctx  +handler  ", factory, &ctrl_cid, h, true, false, ha);
        test_createview("G: fac3-ctx  null-ctx  no-handler", factory, &ctrl_cid, h, false, false, std::ptr::null_mut());

        // Test H: create IComponent(audio_cid), QI for IEditController
        // (tests if "real" controller is inside the component)
        println!("\n--- IComponent QI path ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            let ir=call_init(comp,ha);
            print!("IComponent init={ir:#x}");
            if let Some(ctrl)=qi(comp,&IID_IEDIT_CONTROLLER){
                let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
                print!("  QI->IEditController OK  params={}",pc(ctrl));
                let v=ctrl_create_view(ctrl);
                println!("  createView={v:?}  {}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                if !v.is_null(){call_release(v);}
                call_release(ctrl);
            } else {
                println!("  QI->IEditController FAILED");
            }
            call_term(comp);call_release(comp);
        }

        // Test I: create BOTH IComponent and IEditController simultaneously and keep both alive
        println!("\n--- Simultaneous IComponent + IEditController ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            let ir=call_init(comp,ha);
            print!("IComponent init={ir:#x}");
            // Try to QI IEditController from component WHILE IT IS ALIVE
            if let Some(ctrl_from_comp)=qi(comp,&IID_IEDIT_CONTROLLER){
                let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl_from_comp,8);
                print!("  QI->IEditController OK params={}",pc(ctrl_from_comp));
                let v=ctrl_create_view(ctrl_from_comp);
                println!("  createView={v:?}  {}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                if !v.is_null(){call_release(v);}
                call_release(ctrl_from_comp);
            } else {
                // Create controller separately, keep comp alive
                if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                    let cr=call_init(ctrl,ha);
                    let _ = ctrl_set_handler(ctrl,h);
                    // Try connecting component -> controller via IConnectionPoint
                    let comp_cp=qi(comp,&IID_ICONNECTION_POINT);
                    let ctrl_cp=qi(ctrl,&IID_ICONNECTION_POINT);
                    if let (Some(ccp),Some(tcp))=(comp_cp,ctrl_cp){
                        let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(ccp,3);
                        let r1=f(ccp,tcp);
                        let f2:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(tcp,3);
                        let r2=f2(tcp,ccp);
                        print!("  connect({r1:#x},{r2:#x})");
                        call_release(ccp); call_release(tcp);
                    } else {
                        print!("  no IConnectionPoint");
                    }
                    let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
                    print!("  ctrl init={cr:#x}  params={}",pc(ctrl));
                    let v=ctrl_create_view(ctrl);
                    println!("  createView={v:?}  {}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                    if !v.is_null(){call_release(v);}
                    call_term(ctrl); call_release(ctrl);
                }
            }
            call_term(comp); call_release(comp);
        }

        // Test K: read the bytes of createView function to understand why it returns null
        println!("\n--- createView function disassembly hint ---");
        if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
            let vtbl=*(ctrl as *const *const usize);
            let cv_fn_ptr=*vtbl.add(17) as *const u8;
            print!("  createView @ {:#018x}:", cv_fn_ptr as usize);
            // Print first 64 bytes of the function
            for i in 0..64usize{
                if i%16==0{print!("\n   ");}
                print!(" {:02X}", *cv_fn_ptr.add(i));
            }
            println!();
            // Get DLL base via VirtualQuery
            #[repr(C)]
            struct MemoryBasicInformation{
                base:*mut c_void,alloc_base:*mut c_void,alloc_protect:u32,
                region_size:usize,state:u32,protect:u32,typ:u32,
            }
            let kernel32=libloading::Library::new("kernel32.dll").unwrap();
            let vq:libloading::Symbol<unsafe extern "system" fn(*const c_void,*mut MemoryBasicInformation,usize)->usize>
                =kernel32.get(b"VirtualQuery\0").unwrap();
            let mut mbi=MemoryBasicInformation{base:std::ptr::null_mut(),alloc_base:std::ptr::null_mut(),
                alloc_protect:0,region_size:0,state:0,protect:0,typ:0};
            let sz=std::mem::size_of::<MemoryBasicInformation>();
            vq(cv_fn_ptr as*const c_void,&mut mbi,sz);
            let base=mbi.alloc_base as usize;
            let offset=cv_fn_ptr as usize - base;
            println!("  DLL base={base:#018x}  fn_offset={offset:#010x}");

            // Read suspected "state" field at ctrl+0x60 (derived from lea rbp,[rcx-0x30] + mov rdi,[rbp+0x90])
            let field_at_60=*(ctrl.add(0x60) as *const usize);
            let field_at_70=*(ctrl.add(0x70) as *const usize);
            let field_at_80=*(ctrl.add(0x80) as *const usize);
            let field_at_90=*(ctrl.add(0x90) as *const usize);
            println!("  ctrl[+0x60]={field_at_60:#018x} (+0x70]={field_at_70:#018x} (+0x80]={field_at_80:#018x} (+0x90]={field_at_90:#018x}");
            call_release(ctrl);
        }
        println!("\n--- IEditController vtable dump ---");
        if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
            // Verify the returned pointer IS IEditController via self-QI
            if let Some(ec)=qi(ctrl,&IID_IEDIT_CONTROLLER){
                let same=std::ptr::eq(ctrl,ec);
                println!("  self-QI IEditController: same_ptr={same}");
                call_release(ec);
            } else {
                println!("  self-QI IEditController FAILED! (returned pointer is not IEditController)");
            }
            let vtbl=*(ctrl as *const *const usize);
            for i in 0..20usize{
                println!("  vtbl[{i:2}] = {:#018x}", *vtbl.add(i));
            }
            // Also try calling getParameterInfo(0) even if count is 0
            // getParameterInfo is vtbl[9]: fn(*mut c_void, i32, *mut ParameterInfo) -> i32
            // ParameterInfo is 64 bytes in VST3 SDK
            let mut info=[0u8;64];
            let f:unsafe extern "system" fn(*mut c_void,i32,*mut u8)->i32=vtfn(ctrl,9);
            let r=f(ctrl,0,info.as_mut_ptr());
            println!("  getParameterInfo(0) = {r:#x}");
            call_init(ctrl,std::ptr::null_mut());
            let vtbl2=*(ctrl as *const *const usize);
            println!("  (after init) vtbl == same? {}",std::ptr::eq(vtbl,vtbl2));
            let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
            println!("  (after init) getParameterCount = {}",pc(ctrl));
            call_term(ctrl);call_release(ctrl);
        }

        // Test N: enumerate all interfaces supported by IComponent and IEditController
        println!("\n--- IComponent interface scan ---");
        const KNOWN_IDS: &[(&str, [u8;16])] = &[
            ("IComponent",          vst3_iid(0xE831FF31,0xF2D54301,0x928EBBEE,0x25697802)),
            ("IEditController",     vst3_iid(0xDCD7BBE3,0x7742448D,0xA874AACC,0x979C759E)),
            ("IAudioProcessor",     vst3_iid(0x42043F99,0xB7DA453C,0xA569E79D,0x9AAEC33D)),
            ("IConnectionPoint",    vst3_iid(0x70A4156F,0x6E6E5260,0xACB9BF57,0x7938A98C)),
            ("IEditController2",    vst3_iid(0x7F4EFE59,0xF320BC70,0xDBC09831,0xF4B9C93C)),
            ("IUnitInfo",           vst3_iid(0x3D4BD6B5,0x913A4FD2,0xA886E768,0xA5EB92C1)),
            ("IMidiMapping",        vst3_iid(0xDF0FF9F7,0x268AC671,0xACF9F99B,0xE02A1BEF)),
            ("IAudioPresentationLatency", vst3_iid(0x309ECE78,0xEB7D4FAE,0x8B22D2A,0x14CB01AB)),
            ("IProcessContextRequirements", vst3_iid(0x2A654303,0xEF76415D,0xA91C4427,0x0E61ABCD)),
            ("IKeyswitchController", vst3_iid(0x1BC2C8EA,0xEB817CA4,0x9B8ABEA8,0x4AE35C8F)),
        ];
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            call_init(comp,std::ptr::null_mut());
            for (name,iid) in KNOWN_IDS{
                if let Some(i)=qi(comp,iid){
                    println!("  IComponent supports: {name}");
                    call_release(i);
                }
            }
            call_term(comp);call_release(comp);
        }
        println!("--- IEditController interface scan ---");
        if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
            call_init(ctrl,std::ptr::null_mut());
            for (name,iid) in KNOWN_IDS{
                if let Some(i)=qi(ctrl,iid){
                    println!("  IEditController supports: {name}");
                    call_release(i);
                }
            }
            call_term(ctrl);call_release(ctrl);
        }
        // (tests the theory that the factory needs a live IComponent to properly init the controller)
        println!("\n--- IComponent first, then IEditController (while comp alive) ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            let cir=call_init(comp,ha);
            print!("IComponent init={cir:#x}");
            // NOW create IEditController while IComponent is alive
            if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                let ctrl_ir=call_init(ctrl,ha);
                let field=*(ctrl.add(0x60) as *const usize);
                print!("  ctrl init={ctrl_ir:#x}  field[+0x60]={field:#018x}");
                let _ = ctrl_set_handler(ctrl,h);
                let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
                print!("  params={}",pc(ctrl));
                let v=ctrl_create_view(ctrl);
                println!("  createView={v:?}  {}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                if !v.is_null(){call_release(v);}
                call_term(ctrl);call_release(ctrl);
            }
            call_term(comp);call_release(comp);
        }

        // Test O: init comp+ctrl, wait 500ms, check if field gets populated asynchronously
        // HYPOTHESIS: JUCE plugin registers the AudioProcessor in a global store asynchronously
        println!("\n--- Test O: async field population after init (wait 500ms) ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            let cir=call_init(comp,ha);
            print!("comp init={cir:#x}");
            if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                let ctrl_ir=call_init(ctrl,ha);
                let _ = ctrl_set_handler(ctrl,h);
                let field_before=*(ctrl.add(0x60) as *const usize);
                print!("  ctrl init={ctrl_ir:#x}  field[+0x60]={field_before:#018x} (before wait)");
                std::thread::sleep(std::time::Duration::from_millis(500));
                let field_after=*(ctrl.add(0x60) as *const usize);
                print!("  field[+0x60]={field_after:#018x} (after 500ms)");
                let v=ctrl_create_view(ctrl);
                println!("  createView={}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                if !v.is_null(){call_release(v);}
                call_term(ctrl);call_release(ctrl);
            }
            call_term(comp);call_release(comp);
        }

        // Test P: dump raw memory of comp object (base[0..0x40]) to find inner pointer at +0x28
        println!("\n--- Test P: raw comp object memory dump ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            call_init(comp,ha);
            // dump first 128 bytes of the comp object (IComponent vtable-adjusted pointer)
            println!("  comp ptr = {comp:p}");
            for slot in 0..16usize {
                let val=*(comp.add(slot*8) as *const usize);
                println!("  comp[+{:#04x}] = {val:#018x}", slot*8);
            }
            // also dump ctrl for comparison
            if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                call_init(ctrl,ha);
                println!("  ctrl ptr = {ctrl:p}");
                for slot in 0..16usize {
                    let val=*(ctrl.add(slot*8) as *const usize);
                    println!("  ctrl[+{:#04x}] = {val:#018x}", slot*8);
                }
                // The IEditController vtable is at ctrl[-0x30] (base object start),
                // so also dump the full base object from -0x30 to +0x90
                println!("  --- base object dump (base = ctrl-0x30) ---");
                let base = ctrl.sub(0x30);
                for slot in 0..24usize {
                    let val=*(base.add(slot*8) as *const usize);
                    println!("  base[+{:#04x}] = {val:#018x}", slot*8);
                }
                call_term(ctrl); call_release(ctrl);
            }
            call_term(comp); call_release(comp);
        }

        // Test Q: cross-type creation - try creating ctrl_cid as IComponent and audio_cid as IEditController
        println!("\n--- Test Q: cross-type createInstance ---");
        if let Some(ctrl_as_comp)=fac_create(factory,&ctrl_cid,&IID_ICOMPONENT){
            println!("  ctrl_cid as IComponent: SUCCEEDED  ptr={ctrl_as_comp:p}");
            call_init(ctrl_as_comp, ha);
            // Try to get IEditController from this
            if let Some(ec)=qi(ctrl_as_comp,&IID_IEDIT_CONTROLLER){
                let field=*(ec.add(0x60) as *const usize);
                println!("  QI IEditController: field[+0x60]={field:#018x}");
                let v=ctrl_create_view(ec);
                println!("  createView={}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                if !v.is_null(){call_release(v);}
                call_release(ec);
            }
            call_term(ctrl_as_comp); call_release(ctrl_as_comp);
        } else {
            println!("  ctrl_cid as IComponent: FAILED (ctrl_cid is a distinct class)");
        }
        if let Some(audio_as_ctrl)=fac_create(factory,&audio_cid,&IID_IEDIT_CONTROLLER){
            println!("  audio_cid as IEditController: SUCCEEDED  ptr={audio_as_ctrl:p}");
            call_init(audio_as_ctrl, ha);
            let field=*(audio_as_ctrl.add(0x60) as *const usize);
            let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(audio_as_ctrl,8);
            let params=pc(audio_as_ctrl);
            println!("  params={params}  field[+0x60]={field:#018x}");
            let v=ctrl_create_view(audio_as_ctrl);
            println!("  createView={}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
            if !v.is_null(){call_release(v);}
            call_term(audio_as_ctrl); call_release(audio_as_ctrl);
        } else {
            println!("  audio_cid as IEditController: FAILED");
        }

        // Test R: check field BEFORE ctrl.initialize() — maybe the ctor sets it via global registry
        println!("\n--- Test R: check field immediately after createInstance (before init) ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            // Try without comp init
            println!("  variant 1: no comp.init");
            if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                let field=*(ctrl.add(0x60) as *const usize);
                println!("    ctrl created (no init), field[+0x60]={field:#018x}");
                call_release(ctrl);
            }
            // Now init comp, then create ctrl again
            let cir=call_init(comp,ha);
            println!("  comp init={cir:#x}");
            println!("  variant 2: comp initialized, create ctrl without init");
            if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                let field=*(ctrl.add(0x60) as *const usize);
                println!("    ctrl created (no init), field[+0x60]={field:#018x}");
                let ctrl_ir=call_init(ctrl,ha);
                let field2=*(ctrl.add(0x60) as *const usize);
                println!("    after ctrl.init={ctrl_ir:#x}, field[+0x60]={field2:#018x}");
                call_release(ctrl);
            }
            call_term(comp); call_release(comp);
        }

        // Test S: write ctrl[+0x60] = ctrl (self-pointer) — ctrl[+0x28] is a valid heap ptr
        // so [ctrl+0x60+0x28] would be ctrl[+0x28] = valid object → may not hang!
        // NOTE: if createView hangs (COM STA), we accept timeout and SKIP cleanup to avoid deadlock
        println!("\n--- Test S: ctrl[+0x60]=ctrl self-pointer (2s timeout, no cleanup on hang) ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            let cir=call_init(comp,ha);
            if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                let ctrl_ir=call_init(ctrl,ha);
                let _ = ctrl_set_handler(ctrl,h);
                let ctrl_ptr=ctrl as usize;
                let comp_ptr=comp as usize;
                print!("  comp={cir:#x}  ctrl={ctrl_ir:#x}");
                let field_ptr=ctrl.add(0x60) as *mut usize;
                *field_ptr=ctrl_ptr;
                let inner=*(ctrl.add(0x28) as *const usize);
                print!("  inner(at+0x28)={inner:#018x}");
                let result=std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
                let result2=result.clone();
                std::thread::spawn(move||{
                    let v=ctrl_create_view(ctrl_ptr as *mut c_void);
                    *result2.lock().unwrap()=Some(!v.is_null());
                    if !v.is_null(){call_release(v);}
                });
                std::thread::sleep(std::time::Duration::from_millis(2000));
                let res=result.lock().unwrap().clone();
                match res{
                    Some(true)=>println!("  createView=✓ SUCCESS"),
                    Some(false)=>println!("  createView=✗ null"),
                    None=>println!("  createView=⏱ TIMEOUT (COM message-loop required?)"),
                }
                // Only cleanup if not timed out
                if res.is_some(){
                    *field_ptr=0;
                    call_term(ctrl); call_release(ctrl);
                    call_term(comp); call_release(comp);
                } else {
                    println!("  skipping cleanup to avoid deadlock");
                    let _=comp_ptr; // suppress unused warning
                }
            } else {
                call_term(comp); call_release(comp);
            }
        }

        // Test T: try the "Plugin Compatibility Class" CID from the new Bertom bundle
        // (only applicable to the new bundle; skip if not found)
        // We need to re-enumerate classes from the new bundle if available
        println!("\n--- Test T: new bundle compatibility CID ---");
        let new_bundle=r"C:\Program Files\Common Files\VST3\Bertom_DenoiserClassic.vst3\Contents\x86_64-win\Bertom_DenoiserClassic.vst3";
        if let Ok(new_lib)=libloading::Library::new(new_bundle){
            let gf2:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>=new_lib.get(b"GetPluginFactory\0").unwrap();
            let fac2=gf2();
            let count=fac_count(fac2);
            println!("  new bundle classes: {count}");
            let mut compat_cid=[0u8;16];
            for i in 0..count{
                let mut info=PClassInfo{cid:[0u8;16],_car:0,category:[0u8;32],name:[0u8;64]};
                fac_class_info(fac2,i,&mut info);
                let cat=cstr(&info.category);let name=cstr(&info.name);
                println!("    [{i}] {cat:?} {name:?}  {}",hex16(&info.cid));
                if cat.contains("Compat")||name.contains("Compat"){compat_cid=info.cid;}
            }
            if compat_cid!=[0u8;16]{
                println!("  testing compat_cid={}", hex16(&compat_cid));
                if let Some(ctrl2)=fac_create(fac2,&compat_cid,&IID_IEDIT_CONTROLLER){
                    call_init(ctrl2,ha);
                    let field=*(ctrl2.add(0x60) as *const usize);
                    let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl2,8);
                    println!("  compat as IEditController: params={}  field={field:#018x}",pc(ctrl2));
                    let v=ctrl_create_view(ctrl2);
                    println!("  createView={}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                    if !v.is_null(){call_release(v);}
                    call_term(ctrl2);call_release(ctrl2);
                }
                if let Some(comp2)=fac_create(fac2,&compat_cid,&IID_ICOMPONENT){
                    call_init(comp2,ha);
                    if let Some(ec)=qi(comp2,&IID_IEDIT_CONTROLLER){
                        let field=*(ec.add(0x60) as *const usize);
                        println!("  compat as IComponent QI->IEditController: field={field:#018x}");
                        let v=ctrl_create_view(ec);
                        println!("  createView={}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
                        if !v.is_null(){call_release(v);}
                        call_release(ec);
                    } else {
                        println!("  compat as IComponent QI->IEditController: FAILED");
                    }
                    call_term(comp2);call_release(comp2);
                }
            }
            call_release(fac2);
        } else {
            println!("  new bundle not found, skipping");
        }

        // Test U: comp.getState(stream) → ctrl.setComponentState(stream)
        // HYPOTHESIS: JUCE writes a raw comp pointer into the state stream,
        // and setComponentState reads it to populate juceCompo (= ctrl[+0x60])
        println!("\n--- Test U: comp.getState -> ctrl.setComponentState (pointer transfer) ---");
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            let cir=call_init(comp,ha);
            // comp.getState: IComponent vtbl[13]
            let mut stream=mk_stream();
            let get_state:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(comp,13);
            let gsr=get_state(comp,&mut stream as *mut _ as *mut c_void);
            let data_len=stream.data.len();
            print!("comp init={cir:#x}  getState={gsr:#x}  bytes_written={data_len}");
            if data_len>0{
                print!("  data=[");
                for b in stream.data.iter().take(32){print!("{b:02X} ");}
                if data_len>32{print!("...");}
                print!("]");
            }
            println!();
            if let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER){
                let ctrl_ir=call_init(ctrl,ha);
                let _ = ctrl_set_handler(ctrl,h);
                // Seek stream back to start, then call setComponentState
                bs_seek(&mut stream,0,0,std::ptr::null_mut());
                // setComponentState: IEditController vtbl[5]
                let set_comp_state:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(ctrl,5);
                let scsr=set_comp_state(ctrl,&mut stream as *mut _ as *mut c_void);
                let field=*(ctrl.add(0x60) as *const usize);
                print!("  ctrl init={ctrl_ir:#x}  setComponentState={scsr:#x}  field[+0x60]={field:#018x}");
                if field!=0{
                    println!("  ← NON-NULL! attempting createView...");
                    let ctrl_ptr_u=ctrl as usize;
                    // Run createView in a thread with 3s timeout to avoid COM hang
                    let result=std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
                    let result2=result.clone();
                    std::thread::spawn(move||{
                        let v=ctrl_create_view(ctrl_ptr_u as *mut c_void);
                        *result2.lock().unwrap()=Some(!v.is_null());
                        if !v.is_null(){call_release(v);}
                    });
                    std::thread::sleep(std::time::Duration::from_millis(3000));
                    let res=result.lock().unwrap().clone();
                    match res{
                        Some(true)=>println!("  createView=✓ SUCCESS"),
                        Some(false)=>println!("  createView=✗ null"),
                        None=>println!("  createView=⏱ TIMEOUT"),
                    }
                }else{
                    println!("  field still null, createView skipped");
                }
                call_term(ctrl);call_release(ctrl);
            }
            call_term(comp);call_release(comp);
        }
    }
}

unsafe fn test_createview(label:&str,factory:*mut c_void,ctrl_cid:&[u8;16],h:*mut c_void,use_handler:bool,use_state:bool,ctx:*mut c_void){
    let ctrl=match fac_create(factory,ctrl_cid,&IID_IEDIT_CONTROLLER){
        Some(c)=>c,None=>{println!("[{label}] create failed");return;}
    };
    let ir=call_init(ctrl,ctx);
    print!("[{label}] init={ir:#x}");
    // getParameterCount (vtable[8])
    let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
    print!("  params={}",pc(ctrl));
    if use_handler{let r=ctrl_set_handler(ctrl,h);print!("  handler={r:#x}");}
    if use_state{
        let mut s=mk_stream();
        let sr:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(ctrl,5);
        let r=sr(ctrl,&mut s as *mut _ as *mut c_void);print!("  setState={r:#x}");
    }
    let v=ctrl_create_view(ctrl);
    println!("  createView={v:?}  {}",if !v.is_null(){"✓ SUCCESS"}else{"✗ null"});
    if !v.is_null(){call_release(v);}
    call_term(ctrl);call_release(ctrl);
}
