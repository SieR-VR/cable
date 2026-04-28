//! Test V: find mechanism that sets ctrl[+0x60]
//! - Variant B: comp.setActive before ctrl init
//! - OrilRiver comparison
//! - DLL binary scan for writes to [reg+0x90]
use std::ffi::c_void;
use std::sync::atomic::{AtomicI64, Ordering};

const K_RESULT_OK: i32 = 0;
const K_NO_INTERFACE: i32 = 0x80004002u32 as i32;
const IID_FUNKNOWN: [u8; 16] = [0,0,0,0,0,0,0,0,0xC0,0,0,0,0,0,0,0x46];
const IID_ICOMPONENT_HANDLER: [u8; 16] = vst3_iid(0x93A0BEA3,0x0BD045DB,0x8E890B0C,0xC1E46AC6);
const IID_IBSTREAM:           [u8; 16] = vst3_iid(0xC3BF6EA2,0x3099496A,0x84FB755C,0x90775381);
const IID_ICOMPONENT:         [u8; 16] = vst3_iid(0xE831FF31,0xF2D54301,0x928EBBEE,0x25697802);
const IID_IEDIT_CONTROLLER:   [u8; 16] = vst3_iid(0xDCD7BBE3,0x7742448D,0xA874AACC,0x979C759E);
const IID_IHOST_APPLICATION:  [u8; 16] = vst3_iid(0x58E595CC,0xDB2D4969,0x8B6AAF8C,0x36A664E5);
const IID_IPLUGIN_FACTORY3:   [u8; 16] = vst3_iid(0x4555A2AB,0xC123D4D2,0x94350F8B,0x6A9C4772);
const IID_IAUDIO_PROCESSOR:   [u8; 16] = vst3_iid(0x42043F99,0xB7DA453C,0xA569E79D,0x9AAEC33D);

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

#[repr(C)] struct PClassInfo{cid:[u8;16],_car:i32,category:[u8;32],name:[u8;64]}
unsafe fn fac_count(f:*mut c_void)->i32{let fn_:unsafe extern "system" fn(*mut c_void)->i32=vtfn(f,4);fn_(f)}
unsafe fn fac_class_info(fac:*mut c_void,i:i32,info:*mut PClassInfo)->i32{let f:unsafe extern "system" fn(*mut c_void,i32,*mut PClassInfo)->i32=vtfn(fac,5);f(fac,i,info)}
unsafe fn fac_create(fac:*mut c_void,cid:&[u8;16],iid:&[u8;16])->Option<*mut c_void>{
    let f:unsafe extern "system" fn(*mut c_void,*const u8,*const u8,*mut*mut c_void)->i32=vtfn(fac,6);
    let mut o:*mut c_void=std::ptr::null_mut();
    if f(fac,cid.as_ptr(),iid.as_ptr(),&mut o)==K_RESULT_OK&&!o.is_null(){Some(o)}else{None}
}
unsafe fn comp_ctrl_cid(comp:*mut c_void)->Option<[u8;16]>{
    let f:unsafe extern "system" fn(*mut c_void,*mut[u8;16])->i32=vtfn(comp,5);
    let mut cid=[0u8;16];if f(comp,&mut cid)==K_RESULT_OK{Some(cid)}else{None}
}
// IComponent::setActive is vtbl[10]
unsafe fn comp_set_active(comp:*mut c_void,active:i32)->i32{
    let f:unsafe extern "system" fn(*mut c_void,i32)->i32=vtfn(comp,10);f(comp,active)
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

// IBStream stub
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
    if !wr.is_null(){*wr=n;}K_RESULT_OK
}
unsafe extern "system" fn bs_seek(t:*mut BS,pos:i64,mode:i32,res:*mut i64)->i32{
    let s=&*t;let l=s.data.len()as i64;
    let np=match mode{0=>pos,1=>s.pos.load(Ordering::SeqCst)+pos,2=>l+pos,_=>return 1};
    s.pos.store(np.max(0),Ordering::SeqCst);if !res.is_null(){*res=s.pos.load(Ordering::SeqCst);}K_RESULT_OK
}
unsafe extern "system" fn bs_tell(t:*mut BS,pos:*mut i64)->i32{if !pos.is_null(){*pos=(*t).pos.load(Ordering::SeqCst);}K_RESULT_OK}
static BSVTBL:BSVtbl=BSVtbl{qi:bs_qi,ar:bs_ar,re:bs_re,read:bs_read,write:bs_write,seek:bs_seek,tell:bs_tell};
fn mk_stream()->BS{BS{v:&BSVTBL,pos:AtomicI64::new(0),data:vec![]}}

// IHostApplication stub (silent)
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

struct PluginCtx {
    factory: *mut c_void,
    audio_cid: [u8;16],
    ctrl_cid: [u8;16],
    ha: *mut c_void,
    h: *mut c_void,
}

unsafe fn load_plugin(path:&str, ha:*mut c_void)->Option<(libloading::Library, *mut c_void, [u8;16], [u8;16])>{
    let lib=libloading::Library::new(path).ok()?;
    let get_factory:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>=lib.get(b"GetPluginFactory\0").ok()?;
    let factory=get_factory();
    if let Some(fac3)=qi(factory,&IID_IPLUGIN_FACTORY3){
        let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
        f(fac3,ha);
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
    Some((lib, factory, audio_cid, ctrl_cid))
}

/// Run the standard sequence and check ctrl[+0x60] and createView.
/// sequence: (comp actions before ctrl create), (ctrl actions)
unsafe fn test_sequence(label:&str, factory:*mut c_void, audio_cid:&[u8;16], ctrl_cid:&[u8;16],
    ha:*mut c_void, h:*mut c_void,
    between_comp_init_and_ctrl_create: impl Fn(*mut c_void),
    after_ctrl_init: impl Fn(*mut c_void, *mut c_void)) -> bool {
    println!("\n  [{label}]");
    let Some(comp)=fac_create(factory,audio_cid,&IID_ICOMPONENT) else{
        println!("    comp create FAILED");return false;
    };
    let cir=call_init(comp,ha);
    print!("    comp init={cir:#x}");

    between_comp_init_and_ctrl_create(comp);

    let Some(ctrl)=fac_create(factory,ctrl_cid,&IID_IEDIT_CONTROLLER) else{
        println!("    ctrl create FAILED");
        call_term(comp);call_release(comp);
        return false;
    };
    let ctrl_ir=call_init(ctrl,ha);
    let _ =ctrl_set_handler(ctrl,h);
    print!("  ctrl init={ctrl_ir:#x}");

    after_ctrl_init(comp, ctrl);

    let field=*(ctrl.add(0x60) as *const usize);
    let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
    let params=pc(ctrl);
    println!("  field={field:#018x}  params={params}");

    let success = if field!=0 || params>0 {
        // Try createView in a short window
        let ctrl_usize=ctrl as usize;
        let res=std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
        let res2=res.clone();
        std::thread::spawn(move||{
            let v=ctrl_create_view(ctrl_usize as *mut c_void);
            *res2.lock().unwrap()=Some(!v.is_null());
            if !v.is_null(){call_release(v);}
        });
        std::thread::sleep(std::time::Duration::from_millis(2000));
        let r=*res.lock().unwrap();
        match r{
            Some(true)=>{println!("    createView = SUCCESS");true}
            Some(false)=>{println!("    createView = null");false}
            None=>{println!("    createView = TIMEOUT");false}
        }
    } else {
        println!("    createView = SKIPPED (field null, params=0)");false
    };

    call_term(ctrl);call_release(ctrl);
    call_term(comp);call_release(comp);
    success
}

fn main(){
    let args:Vec<String>=std::env::args().collect();
    let denoiser=args.get(1).map(|s|s.as_str())
        .unwrap_or(r"C:\Program Files\Common Files\VST3\Bertom Denoiser_x64.vst3");
    let orilriver=r"C:\Program Files\Common Files\VST3\OrilRiver.vst3";

    println!("=== Test V: identify ctrl[+0x60] linking mechanism ===\n");

    unsafe{
        let ole32=libloading::Library::new("ole32.dll").unwrap();
        let co_init:libloading::Symbol<unsafe extern "system" fn(*mut c_void,u32)->i32>
            =ole32.get(b"CoInitializeEx\0").unwrap();
        println!("CoInitializeEx(STA) = {:#010x}",co_init(std::ptr::null_mut(),0x2));

        let mut ha_stub=HA{v:&HAVTBL};
        let ha=&mut ha_stub as *mut _ as *mut c_void;
        let mut ch_stub=CH{v:&CHVTBL};
        let h=&mut ch_stub as *mut _ as *mut c_void;

        // ---- Denoiser tests ----
        println!("\n=== Denoiser: {} ===",denoiser);
        let Some((_lib_d, fac_d, audio_d, ctrl_d))=load_plugin(denoiser,ha) else{
            println!("Failed to load Denoiser");return;
        };
        println!("audio_cid={}  ctrl_cid={}",hex16(&audio_d),hex16(&ctrl_d));

        // Variant A: standard (baseline)
        test_sequence("A: standard", fac_d, &audio_d, &ctrl_d, ha, h,
            |_comp|{},
            |_comp,_ctrl|{});

        // Variant B: setActive(true) before creating ctrl
        test_sequence("B: comp.setActive(1) before ctrl create", fac_d, &audio_d, &ctrl_d, ha, h,
            |comp|{let r=comp_set_active(comp,1);print!("  setActive={r:#x}");},
            |_,_|{});

        // Variant C: setActive after ctrl init
        test_sequence("C: setActive after ctrl init", fac_d, &audio_d, &ctrl_d, ha, h,
            |_|{},
            |comp,_ctrl|{let r=comp_set_active(comp,1);print!("  setActive(post)={r:#x}");});

        // Variant D: ctrl created BEFORE comp.init()
        println!("\n  [D: ctrl created before comp.init]");
        if let Some(comp)=fac_create(fac_d,&audio_d,&IID_ICOMPONENT){
            // Don't init comp yet
            if let Some(ctrl)=fac_create(fac_d,&ctrl_d,&IID_IEDIT_CONTROLLER){
                let field_pre=*(ctrl.add(0x60) as *const usize);
                // Now init comp
                let cir=call_init(comp,ha);
                let field_post=*(ctrl.add(0x60) as *const usize);
                // Now init ctrl
                let ctrl_ir=call_init(ctrl,ha);
                let _ =ctrl_set_handler(ctrl,h);
                let field_final=*(ctrl.add(0x60) as *const usize);
                let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
                println!("    comp.init={cir:#x}  ctrl.init={ctrl_ir:#x}");
                println!("    field: pre_comp_init={field_pre:#018x}  post_comp_init={field_post:#018x}  post_ctrl_init={field_final:#018x}  params={}", pc(ctrl));
                call_term(ctrl);call_release(ctrl);
            }
            call_term(comp);call_release(comp);
        }

        // Variant E: use IAudioProcessor QI from comp, call setupProcessing-like
        // (IComponent[5]=getControllerClassId; actually try QI to IAudioProcessor and call setupProcessing)
        println!("\n  [E: QI to IAudioProcessor and activate buses]");
        if let Some(comp)=fac_create(fac_d,&audio_d,&IID_ICOMPONENT){
            let cir=call_init(comp,ha);
            // activateBus: IComponent vtbl[9]
            let act_bus:unsafe extern "system" fn(*mut c_void,i32,i32,i32,i32)->i32=vtfn(comp,9);
            let abr=act_bus(comp,0,1,0,1); // kAudio=0, kInput=1, busIndex=0, state=true
            let abr2=act_bus(comp,0,0,0,1); // kOutput
            print!("    comp.init={cir:#x}  activateBus(in)={abr:#x}  activateBus(out)={abr2:#x}");
            let sar=comp_set_active(comp,1);
            print!("  setActive={sar:#x}");
            if let Some(ctrl)=fac_create(fac_d,&ctrl_d,&IID_IEDIT_CONTROLLER){
                let ctrl_ir=call_init(ctrl,ha);
                let _ =ctrl_set_handler(ctrl,h);
                let field=*(ctrl.add(0x60) as *const usize);
                let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
                println!("  ctrl.init={ctrl_ir:#x}  field={field:#018x}  params={}",pc(ctrl));
                call_term(ctrl);call_release(ctrl);
            }
            call_term(comp);call_release(comp);
        }

        // Variant F: dump full ctrl object memory to find comp pointer
        println!("\n  [F: search comp address in ctrl object memory]");
        if let (Some(comp),Some(ctrl))=(
            fac_create(fac_d,&audio_d,&IID_ICOMPONENT),
            fac_create(fac_d,&ctrl_d,&IID_IEDIT_CONTROLLER)
        ){
            call_init(comp,ha);
            call_init(ctrl,ha);
            let comp_addr=comp as usize;
            let ctrl_base=ctrl.sub(0x30) as *const usize;
            println!("    comp_addr={comp_addr:#018x}  ctrl_base={ctrl_base:?}");
            println!("    scanning 512 bytes of ctrl object for comp address...");
            let mut found_offsets=vec![];
            for i in 0..64usize {  // 64 * 8 = 512 bytes
                let val=*ctrl_base.add(i);
                if val==comp_addr{
                    found_offsets.push(i*8);
                    println!("    FOUND comp_addr at ctrl_base[+{:#x}] = {val:#018x}", i*8);
                }
            }
            if found_offsets.is_empty(){
                println!("    comp address NOT found in ctrl object (first 512 bytes)");
                // Print the object layout anyway
                println!("    ctrl object (64 x 8-byte words):");
                for i in 0..64usize{
                    let val=*ctrl_base.add(i);
                    if val!=0{println!("      [+{:#04x}] = {val:#018x}", i*8);}
                }
            }
            call_term(ctrl);call_release(ctrl);
            call_term(comp);call_release(comp);
        }

        // ---- OrilRiver comparison ----
        println!("\n\n=== OrilRiver (working reference) ===");
        if let Ok(lib_o)=libloading::Library::new(orilriver){
            let gf:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>
                =lib_o.get(b"GetPluginFactory\0").unwrap();
            let fac_o=gf();
            if let Some(fac3)=qi(fac_o,&IID_IPLUGIN_FACTORY3){
                let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
                f(fac3,ha);call_release(fac3);
            }
            let mut audio_o=[0u8;16];
            let mut ctrl_o=[0u8;16];
            for i in 0..fac_count(fac_o){
                let mut info=PClassInfo{cid:[0u8;16],_car:0,category:[0u8;32],name:[0u8;64]};
                fac_class_info(fac_o,i,&mut info);
                let cat=cstr(&info.category);let name=cstr(&info.name);
                println!("  [{i}] {:?} {:?}  {}",cat,name,hex16(&info.cid));
                if cat.starts_with("Audio Module Class"){audio_o=info.cid;}
                if cat.contains("Controller"){ctrl_o=info.cid;}
            }
            if let Some(comp)=fac_create(fac_o,&audio_o,&IID_ICOMPONENT){
                call_init(comp,std::ptr::null_mut());
                if let Some(cid)=comp_ctrl_cid(comp){ctrl_o=cid;}
                call_term(comp);call_release(comp);
            }
            println!("  audio_cid={}  ctrl_cid={}",hex16(&audio_o),hex16(&ctrl_o));
            let same_cid = audio_o==ctrl_o;
            println!("  same CID (single-component): {same_cid}");

            // Standard test
            if let Some(comp)=fac_create(fac_o,&audio_o,&IID_ICOMPONENT){
                let cir=call_init(comp,ha);
                if let Some(ctrl)=fac_create(fac_o,&ctrl_o,&IID_IEDIT_CONTROLLER){
                    let ctrl_ir=call_init(ctrl,ha);
                    let _ =ctrl_set_handler(ctrl,h);
                    let field=*(ctrl.add(0x60) as *const usize);
                    let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ctrl,8);
                    let params=pc(ctrl);
                    println!("  comp.init={cir:#x}  ctrl.init={ctrl_ir:#x}  field={field:#018x}  params={params}");

                    // Object layout dump
                    println!("  ctrl object (first 16 x 8-byte words, non-zero):");
                    let ctrl_base=(ctrl as *const usize).sub(0x30 / 8);
                    for i in 0..32usize{
                        let val=*ctrl_base.add(i);
                        if val!=0{println!("    [+{:#04x}] = {val:#018x}",i*8);}
                    }

                    // Try createView (single thread, short timeout for observation)
                    let v=ctrl_create_view(ctrl);
                    println!("  createView = {}",if !v.is_null(){"non-null (SUCCESS)"}else{"null"});
                    if !v.is_null(){call_release(v);}

                    call_term(ctrl);call_release(ctrl);
                } else if let Some(ec)=qi(comp,&IID_IEDIT_CONTROLLER){
                    println!("  IComponent QI->IEditController succeeded (single-component model)");
                    let field=*(ec.add(0x60) as *const usize);
                    let pc:unsafe extern "system" fn(*mut c_void)->i32=vtfn(ec,8);
                    println!("  comp.init={cir:#x}  field={field:#018x}  params={}",pc(ec));
                    let v=ctrl_create_view(ec);
                    println!("  createView = {}",if !v.is_null(){"non-null (SUCCESS)"}else{"null"});
                    if !v.is_null(){call_release(v);}
                    call_release(ec);
                }
                call_term(comp);call_release(comp);
            }
            call_release(fac_o);
        } else {
            println!("  OrilRiver not found at {orilriver}");
        }

        println!("\nDone.");
    }
}
