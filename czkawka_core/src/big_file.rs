use crate::common::Common;
use crate::common_directory::Directories;
use crate::common_extensions::Extensions;
use crate::common_items::ExcludedItems;
use crate::common_messages::Messages;
use crate::common_traits::{DebugPrint, PrintResults, SaveResults};
use humansize::{file_size_opts as options, FileSize};
use std::collections::BTreeMap;
use std::fs;
use std::fs::{File, Metadata};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub modified_date: u64,
}

/// Info struck with helpful information's about results
pub struct Info {
    pub number_of_checked_files: usize,
    pub number_of_checked_folders: usize,
    pub number_of_ignored_files: usize,
    pub number_of_ignored_things: usize,
    pub taken_space: u64,
    pub number_of_real_files: usize,
}
impl Info {
    pub fn new() -> Info {
        Info {
            number_of_checked_files: 0,
            number_of_checked_folders: 0,
            number_of_ignored_files: 0,
            number_of_ignored_things: 0,
            taken_space: 0,
            number_of_real_files: 0,
        }
    }
}

impl Default for Info {
    fn default() -> Self {
        Self::new()
    }
}

/// Struct with required information's to work
pub struct BigFile {
    text_messages: Messages,
    information: Info,
    big_files: BTreeMap<u64, Vec<FileEntry>>,
    excluded_items: ExcludedItems,
    directories: Directories,
    allowed_extensions: Extensions,
    recursive_search: bool,
    number_of_files_to_check: usize,
}

impl BigFile {
    pub fn new() -> BigFile {
        BigFile {
            text_messages: Default::default(),
            information: Info::new(),
            big_files: Default::default(),
            excluded_items: ExcludedItems::new(),
            directories: Directories::new(),
            allowed_extensions: Extensions::new(),
            recursive_search: true,
            number_of_files_to_check: 50,
        }
    }

    pub fn find_big_files(&mut self) {
        self.optimize_directories();
        self.look_for_big_files();
        self.debug_print();
    }

    pub fn get_big_files(&self) -> &BTreeMap<u64, Vec<FileEntry>> {
        &self.big_files
    }

    pub fn get_text_messages(&self) -> &Messages {
        &self.text_messages
    }

    pub fn get_information(&self) -> &Info {
        &self.information
    }

    pub fn set_recursive_search(&mut self, recursive_search: bool) {
        self.recursive_search = recursive_search;
    }

    /// List of allowed extensions, only files with this extensions will be checking if are duplicates
    pub fn set_allowed_extensions(&mut self, allowed_extensions: String) {
        self.allowed_extensions.set_allowed_extensions(allowed_extensions, &mut self.text_messages);
    }

    fn look_for_big_files(&mut self) {
        let start_time: SystemTime = SystemTime::now();
        let mut folders_to_check: Vec<String> = Vec::with_capacity(1024 * 2); // This should be small enough too not see to big difference and big enough to store most of paths without needing to resize vector

        // Add root folders for finding
        for id in &self.directories.included_directories {
            folders_to_check.push(id.to_string());
        }
        self.information.number_of_checked_folders += folders_to_check.len();

        let mut current_folder: String;
        let mut next_folder: String;
        while !folders_to_check.is_empty() {
            current_folder = folders_to_check.pop().unwrap();

            let read_dir = match fs::read_dir(&current_folder) {
                Ok(t) => t,
                Err(_) => {
                    self.text_messages.warnings.push("Cannot open dir ".to_string() + current_folder.as_str());
                    continue;
                } // Permissions denied
            };
            for entry in read_dir {
                let entry_data = match entry {
                    Ok(t) => t,
                    Err(_) => {
                        self.text_messages.warnings.push("Cannot read entry in dir ".to_string() + current_folder.as_str());
                        continue;
                    } //Permissions denied
                };
                let metadata: Metadata = match entry_data.metadata() {
                    Ok(t) => t,
                    Err(_) => {
                        self.text_messages.warnings.push("Cannot read metadata in dir ".to_string() + current_folder.as_str());
                        continue;
                    } //Permissions denied
                };
                if metadata.is_dir() {
                    self.information.number_of_checked_folders += 1;
                    // if entry_data.file_name().into_string().is_err() { // Probably this can be removed, if crash still will be happens, then uncomment this line
                    //     self.text_messages.warnings.push("Cannot read folder name in dir ".to_string() + current_folder.as_str());
                    //     continue; // Permissions denied
                    // }

                    if !self.recursive_search {
                        continue;
                    }

                    let mut is_excluded_dir = false;
                    next_folder = "".to_owned() + &current_folder + &entry_data.file_name().into_string().unwrap() + "/";

                    for ed in &self.directories.excluded_directories {
                        if next_folder == *ed {
                            is_excluded_dir = true;
                            break;
                        }
                    }
                    if !is_excluded_dir {
                        let mut found_expression: bool = false;
                        for expression in &self.excluded_items.items {
                            if Common::regex_check(expression, &next_folder) {
                                found_expression = true;
                                break;
                            }
                        }
                        if found_expression {
                            break;
                        }
                        folders_to_check.push(next_folder);
                    }
                } else if metadata.is_file() {
                    let mut have_valid_extension: bool;
                    let file_name_lowercase: String = entry_data.file_name().into_string().unwrap().to_lowercase();

                    // Checking allowed extensions
                    if !self.allowed_extensions.file_extensions.is_empty() {
                        have_valid_extension = false;
                        for extension in &self.allowed_extensions.file_extensions {
                            if file_name_lowercase.ends_with((".".to_string() + extension.to_lowercase().as_str()).as_str()) {
                                have_valid_extension = true;
                                break;
                            }
                        }
                    } else {
                        have_valid_extension = true;
                    }

                    // Checking files
                    if have_valid_extension {
                        let current_file_name = "".to_owned() + &current_folder + &entry_data.file_name().into_string().unwrap();

                        // Checking expressions
                        let mut found_expression: bool = false;
                        for expression in &self.excluded_items.items {
                            if Common::regex_check(expression, &current_file_name) {
                                found_expression = true;
                                break;
                            }
                        }
                        if found_expression {
                            break;
                        }

                        // Creating new file entry
                        let fe: FileEntry = FileEntry {
                            path: current_file_name.clone(),
                            size: metadata.len(),
                            modified_date: match metadata.modified() {
                                Ok(t) => match t.duration_since(UNIX_EPOCH) {
                                    Ok(d) => d.as_secs(),
                                    Err(_) => {
                                        self.text_messages.warnings.push(format!("File {} seems to be modified before Unix Epoch.", current_file_name));
                                        0
                                    }
                                },
                                Err(_) => {
                                    self.text_messages.warnings.push("Unable to get modification date from file ".to_string() + current_file_name.as_str());
                                    continue;
                                } // Permissions Denied
                            },
                        };

                        self.big_files.entry(metadata.len()).or_insert_with(Vec::new);
                        self.big_files.get_mut(&metadata.len()).unwrap().push(fe);

                        self.information.number_of_checked_files += 1;
                    } else {
                        self.information.number_of_ignored_files += 1;
                    }
                } else {
                    // Probably this is symbolic links so we are free to ignore this
                    self.information.number_of_ignored_things += 1;
                }
            }
        }

        // Extract n biggest files to new TreeMap
        let mut new_map: BTreeMap<u64, Vec<FileEntry>> = Default::default();

        for (size, vector) in self.big_files.iter().rev() {
            if self.information.number_of_real_files < self.number_of_files_to_check {
                for file in vector {
                    if self.information.number_of_real_files < self.number_of_files_to_check {
                        new_map.entry(*size).or_insert_with(Vec::new);
                        new_map.get_mut(size).unwrap().push(file.clone());
                        self.information.taken_space += size;
                        self.information.number_of_real_files += 1;
                    } else {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        self.big_files = new_map;

        Common::print_time(start_time, SystemTime::now(), "look_for_big_files".to_string());
    }

    pub fn set_number_of_files_to_check(&mut self, number_of_files_to_check: usize) {
        self.number_of_files_to_check = number_of_files_to_check;
    }

    /// Setting excluded items which needs to contains * wildcrard
    /// Are a lot of slower than absolute path, so it should be used to heavy
    pub fn set_excluded_items(&mut self, excluded_items: String) {
        self.excluded_items.set_excluded_items(excluded_items, &mut self.text_messages);
    }

    /// Remove unused entries when included or excluded overlaps with each other or are duplicated etc.
    fn optimize_directories(&mut self) {
        self.directories.optimize_directories(self.recursive_search, &mut self.text_messages);
    }

    /// Setting included directories, at least one must be provided
    pub fn set_included_directory(&mut self, included_directory: String) {
        self.directories.set_included_directory(included_directory, &mut self.text_messages);
    }

    /// Setting absolute path to exclude
    pub fn set_excluded_directory(&mut self, excluded_directory: String) {
        self.directories.set_excluded_directory(excluded_directory, &mut self.text_messages);
    }
}

impl Default for BigFile {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugPrint for BigFile {
    #[allow(dead_code)]
    #[allow(unreachable_code)]
    /// Debugging printing - only available on debug build
    fn debug_print(&self) {
        #[cfg(not(debug_assertions))]
        {
            return;
        }
        println!("---------------DEBUG PRINT---------------");
        println!("### Information's");

        println!("Errors size - {}", self.text_messages.errors.len());
        println!("Warnings size - {}", self.text_messages.warnings.len());
        println!("Messages size - {}", self.text_messages.messages.len());
        println!("Number of checked files - {}", self.information.number_of_checked_files);
        println!("Number of checked folders - {}", self.information.number_of_checked_folders);
        println!("Number of ignored files - {}", self.information.number_of_ignored_files);
        println!("Number of ignored things(like symbolic links) - {}", self.information.number_of_ignored_things);

        println!("### Other");
        println!("Big files size {} in {} groups", self.information.number_of_real_files, self.big_files.len());
        println!("Allowed extensions - {:?}", self.allowed_extensions.file_extensions);
        println!("Excluded items - {:?}", self.excluded_items.items);
        println!("Included directories - {:?}", self.directories.included_directories);
        println!("Excluded directories - {:?}", self.directories.excluded_directories);
        println!("Recursive search - {}", self.recursive_search.to_string());
        println!("Number of files to check - {:?}", self.number_of_files_to_check);
        println!("-----------------------------------------");
    }
}
impl SaveResults for BigFile {
    /// Saving results to provided file
    fn save_results_to_file(&mut self, file_name: &str) -> bool {
        let start_time: SystemTime = SystemTime::now();
        let file_name: String = match file_name {
            "" => "results.txt".to_string(),
            k => k.to_string(),
        };

        let mut file = match File::create(&file_name) {
            Ok(t) => t,
            Err(_) => {
                self.text_messages.errors.push("Failed to create file ".to_string() + file_name.as_str());
                return false;
            }
        };

        match file.write_all(
            format!(
                "Results of searching {:?} with excluded directories {:?} and excluded items {:?}\n",
                self.directories.included_directories, self.directories.excluded_directories, self.excluded_items.items
            )
            .as_bytes(),
        ) {
            Ok(_) => (),
            Err(_) => {
                self.text_messages.errors.push("Failed to save results to file ".to_string() + file_name.as_str());
                return false;
            }
        }

        if self.information.number_of_real_files != 0 {
            file.write_all(format!("{} the biggest files.\n\n", self.information.number_of_real_files).as_bytes()).unwrap();

            for (size, files) in self.big_files.iter().rev() {
                for file_entry in files {
                    file.write_all(format!("{} ({}) - {}\n", size.file_size(options::BINARY).unwrap(), size, file_entry.path.clone()).as_bytes()).unwrap();
                }
            }
        } else {
            file.write_all(b"Not found any empty folders.").unwrap();
        }
        Common::print_time(start_time, SystemTime::now(), "save_results_to_file".to_string());
        true
    }
}
impl PrintResults for BigFile {
    fn print_results(&self) {
        let start_time: SystemTime = SystemTime::now();
        println!("Found {} files which take {}:", self.information.number_of_real_files, self.information.taken_space.file_size(options::BINARY).unwrap());
        for (size, vector) in self.big_files.iter().rev() {
            // TODO Align all to same width
            for entry in vector {
                println!("{} ({} bytes) - {}", size.file_size(options::BINARY).unwrap(), size, entry.path);
            }
        }
        Common::print_time(start_time, SystemTime::now(), "print_entries".to_string());
    }
}
