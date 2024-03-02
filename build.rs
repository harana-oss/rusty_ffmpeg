use std::collections::HashSet;
use std::process::Command;

use bindgen::{self, Bindings, callbacks};
use camino::{Utf8Path as Path, Utf8Path};
use camino::Utf8PathBuf as PathBuf;
use conan2::ConanInstall;
use once_cell::sync::Lazy;

/// All the libs that FFmpeg has
static LIBS: Lazy<[&str; 7]> = Lazy::new(|| {
    [
        "avcodec",
        "avdevice",
        "avfilter",
        "avformat",
        "avutil",
        "swresample",
        "swscale",
    ]
});

/// Whitelist of the headers we want to generate bindings
static HEADERS: Lazy<Vec<PathBuf>> = Lazy::new(|| {
    [
        "libavcodec/avcodec.h",
        "libavcodec/avfft.h",
        "libavcodec/bsf.h",
        "libavcodec/dv_profile.h",
        "libavcodec/vorbis_parser.h",
        "libavdevice/avdevice.h",
        "libavfilter/avfilter.h",
        "libavfilter/buffersink.h",
        "libavfilter/buffersrc.h",
        "libavformat/avformat.h",
        "libavformat/avio.h",
        "libavutil/adler32.h",
        "libavutil/aes.h",
        "libavutil/audio_fifo.h",
        "libavutil/avstring.h",
        "libavutil/avutil.h",
        "libavutil/base64.h",
        "libavutil/blowfish.h",
        "libavutil/bprint.h",
        "libavutil/buffer.h",
        "libavutil/camellia.h",
        "libavutil/cast5.h",
        "libavutil/channel_layout.h",
        "libavutil/cpu.h",
        "libavutil/crc.h",
        "libavutil/dict.h",
        "libavutil/display.h",
        "libavutil/downmix_info.h",
        "libavutil/error.h",
        "libavutil/eval.h",
        "libavutil/fifo.h",
        "libavutil/file.h",
        "libavutil/frame.h",
        "libavutil/hash.h",
        "libavutil/hmac.h",
        "libavutil/imgutils.h",
        "libavutil/lfg.h",
        "libavutil/log.h",
        "libavutil/macros.h",
        "libavutil/mathematics.h",
        "libavutil/md5.h",
        "libavutil/mem.h",
        "libavutil/motion_vector.h",
        "libavutil/murmur3.h",
        "libavutil/opt.h",
        "libavutil/parseutils.h",
        "libavutil/pixdesc.h",
        "libavutil/pixfmt.h",
        "libavutil/random_seed.h",
        "libavutil/rational.h",
        "libavutil/replaygain.h",
        "libavutil/ripemd.h",
        "libavutil/samplefmt.h",
        "libavutil/sha.h",
        "libavutil/sha512.h",
        "libavutil/stereo3d.h",
        "libavutil/threadmessage.h",
        "libavutil/time.h",
        "libavutil/timecode.h",
        "libavutil/twofish.h",
        "libavutil/xtea.h",
        "libswresample/swresample.h",
        "libswscale/swscale.h",
    ]
    .into_iter()
    .map(|x| Path::new(x).into_iter().collect())
    .collect()
});

/// Filter out all symbols in the HashSet, and for others things it will act
/// exactly the same as `CargoCallback`.
#[derive(Debug)]
struct FilterCargoCallbacks {
    emitted_macro: HashSet<String>,
}

impl FilterCargoCallbacks {
    fn new(set: HashSet<String>) -> Self {
        Self { emitted_macro: set }
    }
}

impl callbacks::ParseCallbacks for FilterCargoCallbacks {
    fn will_parse_macro(&self, name: &str) -> callbacks::MacroParsingBehavior {
        if self.emitted_macro.contains(name) {
            callbacks::MacroParsingBehavior::Ignore
        } else {
            callbacks::MacroParsingBehavior::Default
        }
    }
}

fn generate_bindings(ffmpeg_include_dir: Option<&Path>, headers: &[PathBuf]) -> Bindings {
    // Because of the strange `FP_*` in `math.h` https://github.com/rust-lang/rust-bindgen/issues/687
    let filter_callback = FilterCargoCallbacks::new(
        vec![
            "FP_NAN".to_owned(),
            "FP_INFINITE".to_owned(),
            "FP_ZERO".to_owned(),
            "FP_SUBNORMAL".to_owned(),
            "FP_NORMAL".to_owned(),
        ]
        .into_iter()
        .collect(),
    );

    // Bindgen the headers
    headers
        .iter()
        // map header short path to full path
        .map(|header| {
            if let Some(ffmpeg_include_dir) = ffmpeg_include_dir {
                ffmpeg_include_dir.join(header)
            } else {
                header.clone()
            }
        })
        .fold(
            {
                let builder = bindgen::builder()
                    // Force impl Debug if possible(for `AVCodecParameters`)
                    .impl_debug(true)
                    .parse_callbacks(Box::new(filter_callback));
                if let Some(ffmpeg_include_dir) = ffmpeg_include_dir {
                    // Add clang path, for `#include` header finding in bindgen process.
                    builder.clang_arg(format!("-I{}", ffmpeg_include_dir))
                } else {
                    builder
                }
            },
            |builder, header| builder.header(header),
        )
        .generate()
        .expect("Binding generation failed.")
}

fn static_linking(out_dir: &Path, ffmpeg_include_dir: &Path, ffmpeg_libs_dir: &Path) {
    let output_binding_path = &out_dir.join("binding.rs");

    println!("cargo:rustc-link-search=native={}", ffmpeg_libs_dir);
    for library_name in &*LIBS {
        println!("cargo:rustc-link-lib=static={}", library_name);
    }

    generate_bindings(Some(ffmpeg_include_dir), &HEADERS)
        .write_to_file(output_binding_path)
        .expect("Cannot write binding to file.");
}

fn linux_libraries_linking() {
    println!("cargo:rustc-link-lib=X11");
    println!("cargo:rustc-link-lib=va");
    println!("cargo:rustc-link-lib=va-drm");
    println!("cargo:rustc-link-lib=va-x11");
    println!("cargo:rustc-link-lib=vdpau");
}

fn macos_frameworks_linking() {
    println!("cargo:rustc-link-lib=framework=AudioToolbox");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=CoreMedia");
    println!("cargo:rustc-link-lib=framework=CoreVideo");
    println!("cargo:rustc-link-lib=framework=VideoToolbox");
}
  
fn main() {
    #[cfg(target_os = "linux")]
    linux_libraries_linking();

    #[cfg(target_os = "macos")]
    macos_frameworks_linking();

    let conan_dir = match std::env::var("CONAN_HOME").ok() {
      None                   => home::home_dir().unwrap().join(".conan2"),
      Some(dir)       => PathBuf::from(dir).into()
    };

    let build_dir = conan_dir.join("p").join("b");
    ConanInstall::new().build("missing").output_folder(std::path::Path::new("build")).run().parse().emit();

    let ffmpeg_dir = build_dir.read_dir().unwrap().filter(|f| {
        f.as_ref().unwrap().file_name().to_str().unwrap().starts_with("ffmpe")
    }).next().unwrap().unwrap().path().join("p");

    let _ = Command::new("ar").args(["-d", ffmpeg_dir.join("lib").join("libswscale.a").to_str().unwrap(), "half2float.o"]).spawn().unwrap().wait();

    static_linking(
        Path::new("src"),
        Utf8Path::from_path(ffmpeg_dir.join("include").as_path()).unwrap(),
        Utf8Path::from_path(ffmpeg_dir.join("lib").as_path()).unwrap()
    );
}
