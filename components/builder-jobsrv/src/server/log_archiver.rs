// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use VERSION;
use aws_sdk_rust::aws::common::credentials::{DefaultCredentialsProvider, ParametersProvider};
use aws_sdk_rust::aws::common::region::Region;
use aws_sdk_rust::aws::s3::endpoint::{Endpoint, Signature};
use aws_sdk_rust::aws::s3::object::{GetObjectRequest, PutObjectRequest};
use aws_sdk_rust::aws::s3::s3client::S3Client;
use config::ArchiveCfg;
use error::{Result, Error};
use extern_url;
use hyper::client::Client as HyperClient;
use server::log_directory::LogDirectory;
use sha2::{Sha256, Digest};
use std::fs::OpenOptions;
use std::fs;
use std::io::Read;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::str::FromStr;

/// Contract for storage and retrieval of job logs from long-term
/// storage.
///
/// As jobs are running, their log output is collected in files on the
/// job server. Once they are complete, however, we would like to
/// store them elsewhere for safety; the job server should be
/// stateless.
pub trait LogArchiver {

    /// Given a `job_id` and the path to the log output for that job,
    /// places the log in an archive for long-term storage. 
    fn archive(&self, job_id: u64, file_path: &PathBuf) -> Result<()>;

    /// Given a `job_id`, retrieves the log output for that job from
    /// long-term storage.
    fn retrieve(&self, job_id: u64) -> Result<Vec<String>>;
}

/// Create appropriate LogArchiver variant based on configuration
/// values.
// TODO: I'd prefer the individual implementor's constructors to
// actually *be* constructors, instead of arms of an if/else
// statement, but the typechecker stymied me. Come back for this later.
pub fn new(config: ArchiveCfg) -> Result<Box<LogArchiver + 'static>> {
    if config.local {
        // TODO: Only using LogDirectory for the validation
        // logic; we should extract / consolidate this somehow
        let ld = LogDirectory::new(config.local_dir.as_ref().expect("Missing local archive directory!"));
        ld.validate()?;
        Ok(Box::new(LocalArchiver(config.local_dir.unwrap())))
    }
    else {
        let region = Region::from_str(config.region.as_str()).unwrap();
        let param_provider: Option<ParametersProvider>;
        param_provider = Some(ParametersProvider::with_parameters(config.key.expect("Missing S3 key!"),
                                                                  config.secret.expect("Missing S3 secret!").as_str(),
                                                                  None)
                              .unwrap());
        // If given an endpoint, don't use virtual buckets... if not,
        // assume AWS and use virtual buckets.
        //
        // There may be a way to set Minio up to use virtual buckets,
        // but I haven't been able to find it... if there is, then we
        // can go ahead and make this a configuration parameter as well.
        let use_virtual_buckets = !config.endpoint.is_some();

        // Parameterize this if anyone ends up needing V2 signatures
        let signature_type = Signature::V4;
        let final_endpoint = match config.endpoint {
            Some(url) => Some(extern_url::Url::parse(url.as_str())?),
            None => None,
        };
        let user_agent = format!("Habitat-Builder/{}", VERSION);

        let provider = DefaultCredentialsProvider::new(param_provider).unwrap();
        let endpoint = Endpoint::new(region,
                                     signature_type,
                                     final_endpoint,
                                     None,
                                     Some(user_agent),
                                     Some(use_virtual_buckets));

        let client = S3Client::new(provider, endpoint);

        Ok(Box::new(S3Archiver {
            client: client,
            bucket: config.bucket.unwrap(),
        }))
    }
}

////////////////////////////////////////////////////////////////////////
// Local Archive

/// Local archiver variant, which stores job logs in the local
/// filesystem. You are responsible for ensuring this filesystem is
/// backed up / persisted appropriately.
///
/// Generates file paths using a checksum of a job's ID, so files are
/// not all dropped into a single directory. For example, the log for
/// job 722477594578067456 would be stored here:
///
///    /archive/97/6e/48/3c/722477594578067456.log
///
/// where "/archive" is the root of the archive on the filesystem.
pub struct LocalArchiver(PathBuf);

impl LocalArchiver {
    /// Generate the path that a given job's logs will be stored
    /// at. Uses the first 4 bytes of the SHA256 checksums of the ID
    /// to generate a filesystem path that should distribute files so
    /// as not to run afoul of directory limits.
    pub fn archive_path(&self, job_id: u64) -> PathBuf {
        // Generate the checksum
        let mut hasher = Sha256::default();
        hasher.input(job_id.to_string().as_bytes());
        let checksum = hasher.result();

        // Use the hex representation of the first 4 bytes of the
        // checksum to form a new path where the file should be
        // archived.
        let mut new_path = self.0.clone();
        let mut i = checksum.iter();
        for _ in 0..4 {
            let b = i.next().unwrap();
            // 0-pad the representation, e.g. "0a", not "a"
            new_path.push(format!("{:02x}", b));
        }

        new_path.push(format!("{}.log", job_id));
        new_path
    }
}

impl LogArchiver for LocalArchiver {
    fn archive(&self, job_id: u64, file_path: &PathBuf) -> Result<()> {
        let archive_path = self.archive_path(job_id);
        let parent_dir = &archive_path.parent().unwrap();
        fs::create_dir_all(parent_dir)?;
        fs::copy(file_path, &archive_path)?;
        Ok(())
    }

    fn retrieve(&self, job_id: u64) -> Result<Vec<String>> {
        let log_file = self.archive_path(job_id);
        let open = OpenOptions::new().read(true).open(&log_file);
        let mut buffer = Vec::new();
        
        match open {
            Ok(mut file) => {
                file.read_to_end(&mut buffer)?;
                let lines = String::from_utf8_lossy(buffer.as_slice())
                    .lines()
                    .map(|l| l.to_string())
                    .collect();
                Ok(lines)
            }
            Err(e) => {
                warn!("Couldn't open log file {:?}: {:?}", log_file, e);
                Err(Error::IO(e))
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////
// S3 Archive

/// Archiver variant which uses S3 (or an API compatible clone) for
/// log storage.
pub struct S3Archiver {
    client: S3Client<DefaultCredentialsProvider, HyperClient>,
    bucket: String,
}

impl S3Archiver {
    /// Generates the bucket key under which the job log will be
    /// stored.
    fn key(job_id: u64) -> String {
        format!("{}.log", job_id)
    }
}

impl LogArchiver for S3Archiver {
    fn archive(&self, job_id: u64, file_path: &PathBuf) -> Result<()> {
        let mut buffer = Vec::new();
        let mut put_object = PutObjectRequest::default();
        put_object.bucket = self.bucket.clone(); 
        put_object.key = Self::key(job_id);

        let handle = OpenOptions::new().read(true).open(file_path);

        match handle {
            Ok(mut file) => {
                file.read_to_end(&mut buffer)?;
                put_object.body = Some(buffer.as_slice());
            }
            Err(e) => {
                warn!("Could not open {:?} for reading; not archiving! {:?}",
                      file_path,
                      e);
                return Err(Error::from(e));
            }
        }

        // This panics if it can't resolve the URL (e.g.,
        // there's a netsplit, your Minio goes down, S3 goes down (!)).
        // We have to catch it, otherwise no more logs get captured!
        //
        // The code in the S3 library we're currently using isn't
        // UnwindSafe, so we need to deal with that, too.
        let result =
            panic::catch_unwind(AssertUnwindSafe(|| self.client.put_object(&put_object, None)));

        match result {
            Ok(Ok(_)) => Ok(()), // normal result
            Ok(Err(e)) => {
                // This is a "normal", non-panicking error, e.g.,
                // they're configured with a non-existent bucket.
                Err(Error::JobLogArchive(job_id, e))
            }, 
            Err(e) => {
                let source = match e.downcast_ref::<String>() {
                    Some(string) => string.to_string(),
                    None => format!("{:?}", e)
                };
                Err(Error::CaughtPanic(format!("Failure to archive log for job {}", job_id),
                                       source))
            }
        }
    }

    fn retrieve(&self, job_id: u64) -> Result<Vec<String>> {
        let mut request = GetObjectRequest::default();
        request.bucket = self.bucket.clone();
        request.key = Self::key(job_id);

        // As above when uploading a job file, we currently need to
        // catch a potential panic if the object store cannot be reached
        let result =
            panic::catch_unwind(AssertUnwindSafe(|| self.client.get_object(&request, None)));

        let body = match result {
            Ok(Ok(response)) => response.body, // normal result
            Ok(Err(e)) => {
                // This is a "normal", non-panicking error, e.g.,
                // they're configured with a non-existent bucket.
                return Err(Error::JobLogRetrieval(job_id, e))
            }, 
            Err(e) => {
                let source = match e.downcast_ref::<String>() {
                    Some(string) => string.to_string(),
                    None => format!("{:?}", e)
                };
                return Err(Error::CaughtPanic(
                    format!("Failure to retrieve archived log for job {}", job_id),
                    source))
            }
        };
        
        let lines = String::from_utf8_lossy(body.as_slice())
            .lines()
            .map(|l| l.to_string())
            .collect();

        Ok(lines)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_archive_path() {
        let archiver = LocalArchiver(PathBuf::from("/archive"));
        let job_id: u64 = 722543779847979008;
        let expected_path = PathBuf::from("/archive/0a/6b/ef/ac/722543779847979008.log");
        let actual_path = archiver.archive_path(job_id);
        assert_eq!(actual_path,
                   expected_path);
    }
}
