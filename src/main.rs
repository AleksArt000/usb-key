use std::
{
    process::ExitCode,
    env,
    io::Read,
    fs::File,
    fs,
    error::Error,
    fmt::{self, Display},
    path::Path,
};

use sys_mount::
{
    Mount,
    MountFlags,
    SupportedFilesystems,
    Unmount,
    UnmountFlags
};

use libblkid_rs::BlkidProbe;
use libblkid_rs::BlkidCache;
use sha256::{digest, try_digest};

#[derive(Debug)]
struct ExampleError(String);

impl ExampleError 
{
    fn new<D>(d: D) -> Self
    where
        D: Display,
    {
        ExampleError(d.to_string())
    }
}

impl Display for ExampleError 
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result 
    {
        write!(f, "{}", self.0)
    }
}

impl Error for ExampleError {}

struct Config
{
    usb: String,
    key: String,
}

impl Config
{
    fn new(filename: &str) -> Self
    {
        let mut file = File::open(filename).expect("Failed to open the file");
        let mut content = String::new();

        file.read_to_string(&mut content).expect("Failed to read the file");

        let mut usb_value: Option<String> = Option::from(String::new());
        let mut key_value: Option<String> = Option::from(String::new());

        for part in content.split("\n").filter(|s| !s.is_empty())
        {
            let mut field_value = part.split("=");
            let field = field_value.next().unwrap(); // This will be the field name
            let value = field_value.next().unwrap(); // This will be the field value
            
            if field == "USB"
            {
                usb_value = Some(value.to_string());
            }
            else if field == "KEY"
            {
                key_value = Some(value.to_string());
            }
        }

        return Config
        {
            usb: usb_value.unwrap_or("(null)".to_string()),
            key: key_value.unwrap_or("(null)".to_string()),
        };
    }

    fn get_usb(&self) -> &String 
    {
        return &self.usb;
    }

    fn get_key(&self) -> &String 
    {
        return &self.key;
    }
}



fn main() -> Result<(), Box<dyn Error>>
{
    // Reading the config
    let config = Config::new("/etc/usb-key.conf");

    // Get a list of drives
    let binding = fs::read_dir("/dev/disk/by-uuid/");
    let drives = binding.unwrap();


    for mut drive in drives 
    {
        // Setting up the blkid prober to check the UUID
        let mut probe = BlkidProbe::new_from_filename(Path::new(drive.as_mut().unwrap().path().display().to_string().as_str()))?;
        probe.enable_superblocks(true)?;
        probe.enable_partitions(true)?;
        probe.do_safeprobe()?;

        // Get the UUID value, will only work if the drive has one partition
        let uuid = probe.lookup_value("UUID").unwrap();
        let usb_uuid = config.get_usb().as_ref();

        if usb_uuid == uuid
        {
            // Fetch a list of supported file systems.
            // When mounting, a file system will be selected from this.
            let supported = SupportedFilesystems::new().unwrap();

            // Assign the mount point
            let mount_location = "/mnt";

            // Attempt to mount the src device to the dest directory.
            let mount_result = Mount::builder().fstype("ext4").mount(drive.as_mut().unwrap().path().display().to_string().as_str(), mount_location);

            match mount_result 
            {
                Ok(mount) =>
                {
                    // Make the mount temporary, so that it will be unmounted on drop.
                    let mount = mount.into_unmount_drop(UnmountFlags::DETACH);

                    // Get key location
                    let usb_key: &str = config.get_key().as_ref();
                    let key_location = format!("{mount_location}/{usb_key}");
                    
                    // Get sha256 of the key
                    let key_file = Path::new(&key_location);
                    let check = try_digest(key_file).unwrap();
                    println!("{}", check);
                    return Ok(());
                }
                Err(why) => 
                {
                    eprintln!("failed to mount device: {}", why);
                }
            }
        }
        else
        {
            println!("{} does not match the usb", uuid);
        }
    }

    return Ok(());
}