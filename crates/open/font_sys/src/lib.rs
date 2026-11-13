//! Native font discovery. Paths come from the OS cascade; no directory scan or font registration.
use std::path::PathBuf;

pub const SUPPORTED: bool=cfg!(target_vendor="apple");

/// A local font covering a missing UTF-8 shaping cluster, selected for the user's locale.
/// The caller caches the result and maps the file through its existing font database.
pub fn fallback_path(cluster:&str,locale:&str)->Option<PathBuf> {
    if cluster.is_empty() { return None; }
    #[cfg(target_vendor="apple")]
    { apple::fallback_path(cluster,locale) }
    #[cfg(not(target_vendor="apple"))]
    { let _=locale; None }
}

#[cfg(target_vendor="apple")]
mod apple {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;
    use std::ptr::{self,NonNull};

    type Ref=*const c_void;
    #[repr(C)]
    struct Range { location:isize,length:isize }

    #[link(name="CoreFoundation",kind="framework")]
    unsafe extern "C" {
        fn CFRelease(value:Ref);
        fn CFGetTypeID(value:Ref)->usize;
        fn CFURLGetTypeID()->usize;
        fn CFStringCreateWithBytes(alloc:Ref,bytes:*const u8,len:isize,encoding:u32,external:u8)->Ref;
        fn CFStringGetLength(value:Ref)->isize;
        fn CFURLGetFileSystemRepresentation(url:Ref,resolve:u8,buffer:*mut u8,capacity:isize)->u8;
        fn CFArrayGetCount(array:Ref)->isize;
        fn CFArrayGetValueAtIndex(array:Ref,index:isize)->Ref;
        fn CFArrayCreate(alloc:Ref,values:*const Ref,count:isize,callbacks:Ref)->Ref;
        fn CFCharacterSetIsLongCharacterMember(set:Ref,value:u32)->u8;
        fn CFCharacterSetCreateWithCharactersInString(alloc:Ref,string:Ref)->Ref;
        fn CFDictionaryCreate(alloc:Ref,keys:*const Ref,values:*const Ref,count:isize,key_callbacks:Ref,value_callbacks:Ref)->Ref;
        fn CFSetCreate(alloc:Ref,values:*const Ref,count:isize,callbacks:Ref)->Ref;
    }
    #[link(name="CoreText",kind="framework")]
    unsafe extern "C" {
        fn CTFontCreateUIFontForLanguage(kind:u32,size:f64,language:Ref)->Ref;
        fn CTFontCreateForStringWithLanguage(font:Ref,string:Ref,range:Range,language:Ref)->Ref;
        fn CTFontCopyDefaultCascadeListForLanguages(font:Ref,languages:Ref)->Ref;
        fn CTFontCreateWithFontDescriptor(descriptor:Ref,size:f64,matrix:Ref)->Ref;
        fn CTFontCopyCharacterSet(font:Ref)->Ref;
        fn CTFontCopyAvailableTables(font:Ref,options:u32)->Ref;
        fn CTFontDescriptorCreateWithAttributes(attributes:Ref)->Ref;
        fn CTFontDescriptorCreateMatchingFontDescriptors(descriptor:Ref,mandatory:Ref)->Ref;
        fn CTFontCopyAttribute(font:Ref,attribute:Ref)->Ref;
        static kCTFontURLAttribute:Ref;
        static kCTFontCharacterSetAttribute:Ref;
    }

    struct Owned(NonNull<c_void>);
    impl Owned {
        fn new(value:Ref)->Option<Self> { NonNull::new(value.cast_mut()).map(Self) }
        fn raw(&self)->Ref { self.0.as_ptr() }
        fn string(value:&str)->Option<Self> {
            let length=isize::try_from(value.len()).ok()?;
            // CFString copies the borrowed UTF-8 before returning.
            Self::new(unsafe { CFStringCreateWithBytes(ptr::null(),value.as_ptr(),length,0x0800_0100,0) })
        }
    }
    impl Drop for Owned { fn drop(&mut self) { unsafe { CFRelease(self.raw()); } } }

    fn path(font:&Owned)->Option<PathBuf> {
        let tables=Owned::new(unsafe { CTFontCopyAvailableTables(font.raw(),0) })?;
        // Some system faces expose hvgl outlines; Swash needs an OpenType outline or bitmap table.
        let compatible=(0..unsafe { CFArrayGetCount(tables.raw()) }).any(|i| {
            let tag=unsafe { CFArrayGetValueAtIndex(tables.raw(),i) } as usize as u32;
            [u32::from_be_bytes(*b"glyf"),u32::from_be_bytes(*b"CFF "),u32::from_be_bytes(*b"CFF2"),u32::from_be_bytes(*b"sbix"),u32::from_be_bytes(*b"CBDT")].contains(&tag)
        });
        if !compatible { return None; }
        let url=Owned::new(unsafe { CTFontCopyAttribute(font.raw(),kCTFontURLAttribute) })?;
        if unsafe { CFGetTypeID(url.raw())!=CFURLGetTypeID() } { return None; }
        let mut path=[0u8;4096];
        if unsafe { CFURLGetFileSystemRepresentation(url.raw(),1,path.as_mut_ptr(),path.len() as isize) }==0 { return None; }
        let end=path.iter().position(|&b|b==0)?;
        let path=PathBuf::from(std::ffi::OsStr::from_bytes(&path[..end]));
        if !path.is_absolute() || path.file_stem().is_some_and(|name|name=="LastResort") { return None; }
        Some(path)
    }

    fn covers(font:&Owned,cluster:&str)->bool {
        let Some(set)=Owned::new(unsafe { CTFontCopyCharacterSet(font.raw()) }) else { return false };
        cluster.chars().filter(|&c|c!='\u{200d}' && !('\u{fe00}'..='\u{fe0f}').contains(&c)
            && !('\u{e0100}'..='\u{e01ef}').contains(&c))
            .all(|c|unsafe { CFCharacterSetIsLongCharacterMember(set.raw(),c as u32)!=0 })
    }

    fn select(descriptors:Ref,cluster:&str)->Option<PathBuf> {
        for i in 0..unsafe { CFArrayGetCount(descriptors) } {
            let descriptor=unsafe { CFArrayGetValueAtIndex(descriptors,i) };
            let Some(font)=Owned::new(unsafe { CTFontCreateWithFontDescriptor(descriptor,14.0,ptr::null()) }) else { continue };
            if covers(&font,cluster) { if let Some(path)=path(&font) { return Some(path); } }
        }
        None
    }

    pub fn fallback_path(cluster:&str,locale:&str)->Option<PathBuf> {
        let text=Owned::string(cluster)?;
        let language=Owned::string(locale)?;
        let base=Owned::new(unsafe { CTFontCreateUIFontForLanguage(2,14.0,language.raw()) })?;
        let font=Owned::new(unsafe { CTFontCreateForStringWithLanguage(base.raw(),text.raw(),Range { location:0,length:CFStringGetLength(text.raw()) },language.raw()) })?;
        if covers(&font,cluster) { if let Some(path)=path(&font) { return Some(path); } }
        let languages=Owned::new(unsafe { CFArrayCreate(ptr::null(),&language.raw(),1,ptr::null()) })?;
        let cascade=Owned::new(unsafe { CTFontCopyDefaultCascadeListForLanguages(base.raw(),languages.raw()) })?;
        if let Some(path)=select(cascade.raw(),cluster) { return Some(path); }
        // Ask the OS font registry when the preferred cascade has no supported outline format.
        let charset=Owned::new(unsafe { CFCharacterSetCreateWithCharactersInString(ptr::null(),text.raw()) })?;
        let key=unsafe { kCTFontCharacterSetAttribute };
        let attributes=Owned::new(unsafe { CFDictionaryCreate(ptr::null(),&key,&charset.raw(),1,ptr::null(),ptr::null()) })?;
        let descriptor=Owned::new(unsafe { CTFontDescriptorCreateWithAttributes(attributes.raw()) })?;
        let mandatory=Owned::new(unsafe { CFSetCreate(ptr::null(),&key,1,ptr::null()) })?;
        let matches=Owned::new(unsafe { CTFontDescriptorCreateMatchingFontDescriptors(descriptor.raw(),mandatory.raw()) })?;
        select(matches.raw(),cluster)
    }
}

#[cfg(all(test,target_os="macos"))]
mod tests {
    #[test]
    fn native_cascade_returns_local_fonts_for_scripts_and_emoji() {
        for (cluster,locale) in [("漢","ja-JP"),("لا","ar"),("Ж","ru"),("⌘","en-US"),("😀","en-US"),("👩‍💻","en-US")] {
            let path=super::fallback_path(cluster,locale).expect("system cascade font");
            assert!(path.is_absolute() && path.is_file(),"{cluster}: {path:?}");
        }
        assert!(super::fallback_path("","en-US").is_none());
    }
}
