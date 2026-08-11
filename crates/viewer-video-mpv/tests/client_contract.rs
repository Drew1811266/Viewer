use std::{
    ffi::{CStr, CString, c_char, c_int, c_ulong, c_void},
    path::Path,
    ptr,
    sync::{Mutex, OnceLock},
};

use viewer_video_mpv::{
    FrameDirection, MpvApi, MpvClient, PlaybackRate,
    ffi::{MpvEvent, MpvFormat, MpvHandle},
};

#[derive(Debug, PartialEq)]
enum Call {
    Create,
    Option(String, String),
    Initialize,
    Command(Vec<String>),
    PropertyFlag(String, bool),
    PropertyDouble(String, f64),
    PropertyRead(String),
    Destroy,
}

fn calls() -> &'static Mutex<Vec<Call>> {
    static CALLS: OnceLock<Mutex<Vec<Call>>> = OnceLock::new();
    CALLS.get_or_init(|| Mutex::new(Vec::new()))
}

unsafe extern "C" fn client_api_version() -> c_ulong {
    ((2_u32 << 16) | 5).into()
}

unsafe extern "C" fn create() -> *mut MpvHandle {
    calls().lock().unwrap().push(Call::Create);
    ptr::dangling_mut::<MpvHandle>()
}

unsafe extern "C" fn initialize(_: *mut MpvHandle) -> c_int {
    calls().lock().unwrap().push(Call::Initialize);
    0
}

unsafe extern "C" fn terminate_destroy(_: *mut MpvHandle) {
    calls().lock().unwrap().push(Call::Destroy);
}

unsafe extern "C" fn set_option_string(
    _: *mut MpvHandle,
    name: *const c_char,
    value: *const c_char,
) -> c_int {
    let name = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned();
    let value = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned();
    calls().lock().unwrap().push(Call::Option(name, value));
    0
}

unsafe extern "C" fn command(_: *mut MpvHandle, args: *const *const c_char) -> c_int {
    let mut values = Vec::new();
    let mut offset = 0;
    loop {
        let argument = unsafe { *args.add(offset) };
        if argument.is_null() {
            break;
        }
        values.push(
            unsafe { CStr::from_ptr(argument) }
                .to_string_lossy()
                .into_owned(),
        );
        offset += 1;
    }
    calls().lock().unwrap().push(Call::Command(values));
    0
}

unsafe extern "C" fn set_property(
    _: *mut MpvHandle,
    name: *const c_char,
    format: MpvFormat,
    data: *mut c_void,
) -> c_int {
    let name = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned();
    let call = match format {
        3 => Call::PropertyFlag(name, unsafe { *(data.cast::<c_int>()) } != 0),
        5 => Call::PropertyDouble(name, unsafe { *(data.cast::<f64>()) }),
        _ => panic!("unexpected property format {format}"),
    };
    calls().lock().unwrap().push(call);
    0
}

unsafe extern "C" fn get_property(
    _: *mut MpvHandle,
    name: *const c_char,
    format: MpvFormat,
    data: *mut c_void,
) -> c_int {
    let name = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned();
    if name == "time-pos" {
        assert_eq!(format, 5);
        calls().lock().unwrap().push(Call::PropertyRead(name));
        unsafe { *data.cast::<f64>() = 1.25 };
        return 0;
    }
    assert_eq!(format, 1);
    let value = match name.as_str() {
        "hwdec-current" => "videotoolbox",
        "current-vo" => "libmpv",
        _ => panic!("unexpected property read {name}"),
    };
    calls().lock().unwrap().push(Call::PropertyRead(name));
    unsafe { *data.cast::<*mut c_char>() = CString::new(value).unwrap().into_raw() };
    0
}

unsafe extern "C" fn free(data: *mut c_void) {
    drop(unsafe { CString::from_raw(data.cast::<c_char>()) });
}

unsafe extern "C" fn observe_property(
    _: *mut MpvHandle,
    _: u64,
    _: *const c_char,
    _: MpvFormat,
) -> c_int {
    0
}

unsafe extern "C" fn wait_event(_: *mut MpvHandle, _: f64) -> *const MpvEvent {
    ptr::null()
}

fn fake_api() -> MpvApi {
    MpvApi {
        client_api_version,
        create,
        initialize,
        terminate_destroy,
        set_option_string,
        command,
        set_property,
        get_property,
        free,
        observe_property,
        wait_event,
    }
}

#[test]
fn public_client_contract_is_isolated_and_typed() {
    calls().lock().unwrap().clear();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let canonical_path = path.canonicalize().unwrap();

    let mut client = unsafe { MpvClient::from_api(fake_api()) }.unwrap();
    client.initialize_for_rendering().unwrap();
    assert!(
        !calls()
            .lock()
            .unwrap()
            .iter()
            .any(|call| matches!(call, Call::Command(_))),
        "render initialization must not load media before a render context exists"
    );
    client.open_local_file(&canonical_path).unwrap();
    client.play().unwrap();
    client.pause().unwrap();
    client.frame_step(FrameDirection::Forward).unwrap();
    client.frame_step(FrameDirection::Backward).unwrap();
    client.set_volume_percent(67).unwrap();
    client.set_muted(true).unwrap();
    client.set_rate(PlaybackRate::OneAndHalf).unwrap();
    assert_eq!(
        client.active_hardware_decoder().unwrap().as_deref(),
        Some("videotoolbox")
    );
    assert_eq!(
        client.active_video_output().unwrap().as_deref(),
        Some("libmpv")
    );
    assert_eq!(client.current_playback_time_us().unwrap(), Some(1_250_000));
    drop(client);

    let calls = calls().lock().unwrap();
    let initialize_index = calls
        .iter()
        .position(|call| call == &Call::Initialize)
        .unwrap();
    let load_index = calls
        .iter()
        .position(|call| matches!(call, Call::Command(args) if args.first().map(String::as_str) == Some("loadfile")))
        .unwrap();
    assert!(initialize_index < load_index);

    for (name, value) in [
        ("vo", "libmpv"),
        ("hwdec", "auto-safe"),
        ("config", "no"),
        ("load-scripts", "no"),
        ("input-default-bindings", "no"),
        ("ytdl", "no"),
        ("autoload-files", "no"),
        ("access-references", "no"),
        ("load-unsafe-playlists", "no"),
        ("demuxer-lavf-o", "protocol_whitelist=%16%file,crypto,data"),
        ("sid", "no"),
        ("sub-auto", "no"),
        ("audio-file-auto", "no"),
        ("cover-art-auto", "no"),
        ("aid", "auto"),
        ("loop-file", "no"),
        ("network-timeout", "0"),
    ] {
        let option_index = calls
            .iter()
            .position(|call| call == &Call::Option(name.to_owned(), value.to_owned()))
            .unwrap_or_else(|| panic!("missing option {name}={value}"));
        assert!(option_index < initialize_index, "{name} was set too late");
    }

    assert!(calls.contains(&Call::Command(vec![
        "loadfile".to_owned(),
        canonical_path.to_string_lossy().into_owned(),
        "replace".to_owned(),
    ])));
    assert!(calls.contains(&Call::PropertyFlag("pause".to_owned(), false)));
    assert!(calls.contains(&Call::PropertyFlag("pause".to_owned(), true)));
    assert!(calls.contains(&Call::Command(vec!["frame-step".to_owned()])));
    assert!(calls.contains(&Call::Command(vec!["frame-back-step".to_owned()])));
    assert!(calls.contains(&Call::PropertyDouble("volume".to_owned(), 67.0)));
    assert!(calls.contains(&Call::PropertyFlag("mute".to_owned(), true)));
    assert!(calls.contains(&Call::PropertyDouble("speed".to_owned(), 1.5)));
    assert!(calls.contains(&Call::PropertyRead("hwdec-current".to_owned())));
    assert!(calls.contains(&Call::PropertyRead("current-vo".to_owned())));
    assert!(calls.contains(&Call::PropertyRead("time-pos".to_owned())));
    assert_eq!(calls.last(), Some(&Call::Destroy));
}
