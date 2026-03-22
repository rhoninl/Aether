use std::fs;
use std::path::PathBuf;

use crate::config::{BuildConfig, QUEST_MIN_SDK_VERSION, QUEST_TARGET_SDK_VERSION};
use crate::BuildError;

const QUEST_VR_CATEGORY: &str = "com.oculus.intent.category.VR";
const QUEST_HEADTRACKING_FEATURE: &str = "android.hardware.vr.headtracking";
const QUEST_HAND_TRACKING_FEATURE: &str = "com.oculus.handtracking";

/// Generate an AndroidManifest.xml for Quest VR.
pub fn generate(config: &BuildConfig) -> Result<PathBuf, BuildError> {
    let build_dir = config.build_dir();
    fs::create_dir_all(&build_dir).map_err(|e| BuildError::ManifestGenerationFailed {
        reason: format!("failed to create build directory: {e}"),
    })?;

    let xml = generate_manifest_xml(
        &config.package_name(),
        &config.app_name,
        QUEST_MIN_SDK_VERSION,
        QUEST_TARGET_SDK_VERSION,
    );

    let manifest_path = build_dir.join("AndroidManifest.xml");
    fs::write(&manifest_path, &xml).map_err(|e| BuildError::ManifestGenerationFailed {
        reason: format!("failed to write AndroidManifest.xml: {e}"),
    })?;

    Ok(manifest_path)
}

/// Generate the AndroidManifest.xml content string.
pub fn generate_manifest_xml(
    package_name: &str,
    app_name: &str,
    min_sdk: u32,
    target_sdk: u32,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{package_name}"
    android:versionCode="1"
    android:versionName="0.1.0">

    <uses-sdk
        android:minSdkVersion="{min_sdk}"
        android:targetSdkVersion="{target_sdk}" />

    <uses-feature
        android:name="{QUEST_HEADTRACKING_FEATURE}"
        android:required="true"
        android:version="1" />
    <uses-feature
        android:name="{QUEST_HAND_TRACKING_FEATURE}"
        android:required="false" />
    <uses-feature
        android:glEsVersion="0x00030001"
        android:required="true" />

    <application
        android:hasCode="false"
        android:extractNativeLibs="true"
        android:label="{app_name}">
        <meta-data
            android:name="com.oculus.supportedDevices"
            android:value="quest3|questpro|quest2" />
        <activity
            android:name="android.app.NativeActivity"
            android:configChanges="density|keyboard|keyboardHidden|navigation|orientation|screenLayout|screenSize|uiMode"
            android:screenOrientation="landscape"
            android:launchMode="singleTask"
            android:exported="true"
            android:theme="@android:style/Theme.Black.NoTitleBar.Fullscreen">
            <meta-data
                android:name="android.app.lib_name"
                android:value="main" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
                <category android:name="{QUEST_VR_CATEGORY}" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_manifest() -> String {
        generate_manifest_xml("com.aether.testapp", "TestApp", 29, 32)
    }

    #[test]
    fn manifest_contains_vr_category() {
        let xml = test_manifest();
        assert!(
            xml.contains(QUEST_VR_CATEGORY),
            "manifest missing VR category"
        );
    }

    #[test]
    fn manifest_contains_headtracking_feature() {
        let xml = test_manifest();
        assert!(
            xml.contains(QUEST_HEADTRACKING_FEATURE),
            "manifest missing headtracking feature"
        );
    }

    #[test]
    fn manifest_contains_hand_tracking_feature() {
        let xml = test_manifest();
        assert!(
            xml.contains(QUEST_HAND_TRACKING_FEATURE),
            "manifest missing hand tracking feature"
        );
    }

    #[test]
    fn manifest_has_native_activity() {
        let xml = test_manifest();
        assert!(
            xml.contains("android.app.NativeActivity"),
            "manifest missing NativeActivity"
        );
    }

    #[test]
    fn manifest_has_no_code_flag() {
        let xml = test_manifest();
        assert!(
            xml.contains("android:hasCode=\"false\""),
            "manifest should declare hasCode=false for NativeActivity"
        );
    }

    #[test]
    fn manifest_min_sdk_version() {
        let xml = test_manifest();
        assert!(
            xml.contains("android:minSdkVersion=\"29\""),
            "manifest should have minSdkVersion=29"
        );
    }

    #[test]
    fn manifest_target_sdk_version() {
        let xml = test_manifest();
        assert!(
            xml.contains("android:targetSdkVersion=\"32\""),
            "manifest should have targetSdkVersion=32"
        );
    }

    #[test]
    fn manifest_lib_name_is_main() {
        let xml = test_manifest();
        assert!(
            xml.contains("android:value=\"main\""),
            "manifest lib_name should be 'main'"
        );
    }

    #[test]
    fn manifest_app_name_substitution() {
        let xml = generate_manifest_xml("com.aether.myworld", "My Cool World", 29, 32);
        assert!(
            xml.contains("android:label=\"My Cool World\""),
            "manifest should contain app name in label"
        );
    }

    #[test]
    fn manifest_package_name_format() {
        let xml = test_manifest();
        assert!(
            xml.contains("package=\"com.aether.testapp\""),
            "manifest should contain package name"
        );
    }

    #[test]
    fn manifest_is_valid_xml_structure() {
        let xml = test_manifest();
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.contains("<manifest"));
        assert!(xml.contains("</manifest>"));
        assert!(xml.contains("<application"));
        assert!(xml.contains("</application>"));
        assert!(xml.contains("<activity"));
        assert!(xml.contains("</activity>"));
    }

    #[test]
    fn manifest_contains_gles_requirement() {
        let xml = test_manifest();
        assert!(
            xml.contains("android:glEsVersion"),
            "manifest should require OpenGL ES"
        );
    }

    #[test]
    fn manifest_landscape_orientation() {
        let xml = test_manifest();
        assert!(
            xml.contains("android:screenOrientation=\"landscape\""),
            "Quest apps should be landscape"
        );
    }

    #[test]
    fn generate_writes_file() {
        let tmp = TempDir::new().unwrap();
        let mut config = crate::config::BuildConfig::new(
            tmp.path().to_path_buf(),
            crate::config::BuildTarget::Quest,
            crate::config::BuildProfile::Debug,
        );
        config.app_name = "testapp".to_string();

        let path = generate(&config).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains(QUEST_VR_CATEGORY));
    }
}
