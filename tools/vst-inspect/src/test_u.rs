//! Minimal Test U: comp.getState(stream) → ctrl.setComponentState(stream) → createView
//! Tests if JUCE transfers the component pointer via state stream.
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

const fn vst3_iid(l1:u32,l2:u32,l3:u32,l4:u32)->[u8;16]{
    [(l1&0xFF)as u8,((l1>>8)&0xFF)as u8,((l1>>16)&0xFF)as u8,((l1>>24)&0xFF)as u8,
     ((l2>>16)&0xFF)as u8,((l2>>24)&0xFF)as u8,(l2&0xFF)as u8,((l2>>8)&0xFF)as u8,
     ((l3>>24)&0xFF)as u8,((l3>>16)&0xFF)as u8,((l3>>8)&0xFF)as u8,(l3&0xFF)as u8,
     ((l4>>24)&0xFF)as u8,((l4>>16)&0xFF)as u8,((l4>>8)&0xFF)as u8,(l4&0xFF)as u8]
}
fn hex16(b:&[u8;16])->String{b.iter().map(|x|format!("{x:02X}")).collect::<Vec<_>>().join("")}
fn cstr(b:&[u8])->String{let e=b.iter().position(|&x|x==0).unwrap_or(b.len());String::from_utf8_lossy(&b[..e]).into_owned()}

unsafe fn vtfn<F:Copy>(obj:*mut c_void,idx:usize)->F{let v=*(obj as *mut*const usize);std::mem::transmute_copy(&*v.add(idx))}
unsafe fn call_qi(o:*mut c_void,iid:&[u8;16],out:*mut*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*const u8,*mut*mut c_void)->i32=vtfn(o,0);f(o,iid.as_ptr(),out)}
unsafe fn call_release(o:*mut c_void)->u32{let f:unsafe extern "system" fn(*mut c_void)->u32=vtfn(o,2);f(o)}
unsafe fn call_init(o:*mut c_void,ctx:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(o,3);f(o,ctx)}
unsafe fn call_term(o:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void)->i32=vtfn(o,4);f(o)}
unsafe fn ctrl_set_handler(c:*mut c_void,h:*mut c_void)->i32{let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(c,16);f(c,h)}
unsafe fn ctrl_create_view(c:*mut c_void)->*mut c_void{let f:unsafe extern "system" fn(*mut c_void,*const i8)->*mut c_void=vtfn(c,17);f(c,b"editor\0".as_ptr() as*const i8)}
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

// IBStream stub with real storage
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
unsafe extern "system" fn ha_crei(_:*mut HA,_:*const u8,_:*const u8,o:*mut*mut c_void)->i32{
    *o=std::ptr::null_mut();K_NO_INTERFACE
}
static HAVTBL:HAVtbl=HAVtbl{qi:ha_qi,ar:ha_ar,re:ha_re,name:ha_name,crei:ha_crei};

fn main(){
    let args:Vec<String>=std::env::args().collect();
    let path=args.get(1).map(|s|s.as_str())
        .unwrap_or(r"C:\Program Files\Common Files\VST3\Bertom Denoiser_x64.vst3");
    println!("=== Test U: comp.getState → ctrl.setComponentState → createView ===");
    println!("Plugin: {path}\n");

    unsafe{
        let ole32=libloading::Library::new("ole32.dll").unwrap();
        let co_init:libloading::Symbol<unsafe extern "system" fn(*mut c_void,u32)->i32>
            =ole32.get(b"CoInitializeEx\0").unwrap();
        let hr=co_init(std::ptr::null_mut(),0x2); // STA
        println!("CoInitializeEx(STA) = {hr:#010x}");

        let lib=libloading::Library::new(path).expect("failed to load plugin");
        let get_factory:libloading::Symbol<unsafe extern "system" fn()->*mut c_void>
            =lib.get(b"GetPluginFactory\0").expect("GetPluginFactory not found");
        let factory=get_factory();

        // Set factory host context
        let mut ha_stub=HA{v:&HAVTBL};
        let ha=&mut ha_stub as *mut _ as *mut c_void;
        if let Some(fac3)=qi(factory,&IID_IPLUGIN_FACTORY3){
            let f:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(fac3,9);
            let r=f(fac3,ha);
            println!("IPluginFactory3::setHostContext = {r:#x}");
            call_release(fac3);
        }

        // Enumerate classes
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
        // Refine ctrl_cid via IComponent::getControllerClassId
        if let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT){
            call_init(comp,std::ptr::null_mut());
            if let Some(cid)=comp_ctrl_cid(comp){
                println!("  getControllerClassId = {}",hex16(&cid));
                ctrl_cid=cid;
            }
            call_term(comp);call_release(comp);
        }
        println!("\naudio_cid = {}",hex16(&audio_cid));
        println!("ctrl_cid  = {}",hex16(&ctrl_cid));

        // Set up handler
        let mut ch_stub=CH{v:&CHVTBL};
        let h=&mut ch_stub as *mut _ as *mut c_void;

        println!("\n--- Test U: comp.getState -> ctrl.setComponentState ---");

        let Some(comp)=fac_create(factory,&audio_cid,&IID_ICOMPONENT) else {
            println!("FAILED: could not create IComponent");
            return;
        };
        let cir=call_init(comp,ha);
        println!("comp created, init={cir:#x}");

        // Step 1: comp.getState
        let mut stream=mk_stream();
        let get_state:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(comp,13);
        let gsr=get_state(comp,&mut stream as *mut _ as *mut c_void);
        let data_len=stream.data.len();
        println!("comp.getState = {gsr:#x},  bytes_written = {data_len}");
        if data_len>0{
            print!("  data (first 64 bytes): [");
            for b in stream.data.iter().take(64){print!("{b:02X} ");}
            if data_len>64{print!("...");}
            println!("]");
            // Show as pointer-sized chunks
            println!("  as u64 words:");
            for chunk in stream.data.chunks(8).take(8){
                let mut arr=[0u8;8];
                let l=chunk.len().min(8);
                arr[..l].copy_from_slice(&chunk[..l]);
                let v=u64::from_le_bytes(arr);
                println!("    {v:#018x}");
            }
        } else {
            println!("  EMPTY STATE (getState wrote nothing!)");
        }

        let Some(ctrl)=fac_create(factory,&ctrl_cid,&IID_IEDIT_CONTROLLER) else {
            println!("FAILED: could not create IEditController");
            call_term(comp);call_release(comp);
            return;
        };
        let ctrl_ir=call_init(ctrl,ha);
        let _ = ctrl_set_handler(ctrl,h);
        println!("ctrl created, init={ctrl_ir:#x}");

        // Field before setComponentState
        let field_before=*(ctrl.add(0x60) as *const usize);
        println!("ctrl[+0x60] before setComponentState = {field_before:#018x}");

        // Step 2: seek stream to beginning and call setComponentState
        bs_seek(&mut stream,0,0,std::ptr::null_mut());
        let set_comp_state:unsafe extern "system" fn(*mut c_void,*mut c_void)->i32=vtfn(ctrl,5);
        let scsr=set_comp_state(ctrl,&mut stream as *mut _ as *mut c_void);
        println!("ctrl.setComponentState = {scsr:#x}");

        let field_after=*(ctrl.add(0x60) as *const usize);
        println!("ctrl[+0x60] after  setComponentState = {field_after:#018x}");

        if field_after!=0 {
            println!("\nFIELD IS NON-NULL! Attempting createView (3s timeout)...");
            let ctrl_usize=ctrl as usize;
            let result=std::sync::Arc::new(std::sync::Mutex::new(None::<bool>));
            let result2=result.clone();
            std::thread::spawn(move||{
                let v=ctrl_create_view(ctrl_usize as *mut c_void);
                *result2.lock().unwrap()=Some(!v.is_null());
                if !v.is_null(){call_release(v);}
            });
            std::thread::sleep(std::time::Duration::from_millis(3000));
            let res = *result.lock().unwrap();
            match res{
                Some(true)=>println!("createView = SUCCESS"),
                Some(false)=>println!("createView = null"),
                None=>println!("createView = TIMEOUT (COM STA needed)"),
            }
        } else {
            println!("\nField is still null — hypothesis INCORRECT or getState wrote no data.");
            // Try a different approach: write comp pointer directly to stream then setComponentState
            if data_len==0{
                println!("\n--- Variant: manually write comp pointer to stream, then setComponentState ---");
                let comp_addr=comp as u64;
                println!("  Writing comp pointer {comp_addr:#018x} into stream...");
                let mut stream2=mk_stream();
                let addr_bytes=comp_addr.to_le_bytes();
                let n=addr_bytes.len() as i32;
                let mut bw:i32=0;
                bs_write(&mut stream2,addr_bytes.as_ptr() as *mut c_void,n,&mut bw);
                println!("  bytes_written={bw}");
                bs_seek(&mut stream2,0,0,std::ptr::null_mut());
                let scsr2=set_comp_state(ctrl,&mut stream2 as *mut _ as *mut c_void);
                let field2=*(ctrl.add(0x60) as *const usize);
                println!("  setComponentState(comp_addr stream) = {scsr2:#x}  field={field2:#018x}");
            }
        }

        // Cleanup
        call_term(ctrl);call_release(ctrl);
        call_term(comp);call_release(comp);
        println!("\nDone.");
    }
}
