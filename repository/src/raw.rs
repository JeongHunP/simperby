use super::*;
use async_trait::async_trait;
use simperby_common::reserved::ReservedState;
use thiserror::Error;
use git2::{Repository, BranchType, Oid, ObjectType};
use std::str;
use std::convert::TryFrom;

#[derive(Error, Debug)]
pub enum Error {
    #[error("git2 error: {0}")]
    Git2Error(git2::Error),
    /// When the assumption of the method (e.g., there is no merge commit) is violated.
    #[error("the repository is invalid: {0}")]
    InvalidRepository(String),
    #[error("unknown error: {0}")]
    Unknown(String),
}

impl From<git2::Error> for Error {
    fn from(e: git2::Error) -> Self {
        Error::Git2Error(e)
    }
}

/// A commit without any diff on non-reserved area.
#[derive(Debug, Clone)]
pub struct SemanticCommit {
    pub title: String,
    pub body: String,
    /// (If this commit made any change) the new reserved state.
    pub reserved_state: Option<ReservedState>,
}

/// A raw handle for the local repository.
///
/// It automatically locks the repository once created.
#[async_trait]
pub trait RawRepository {
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
    fn init(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    // Loads an exisitng repository.
    fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    fn list_branches(&self) -> Result<Vec<Branch>, Error>;

    /// Creates a branch on the commit.
    fn create_branch(
        &self,
        branch_name: &Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error>;

    /// Gets the commit that the branch points to.
    fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error>;

    /// Gets the list of branches from the commit.
    fn get_branches(&self, commit_hash: &CommitHash) -> Result<Vec<Branch>, Error>;

    /// Moves the branch.
    fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash)
        -> Result<(), Error>;

    /// Deletes the branch.
    fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error>;

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    fn list_tags(&self) -> Result<Vec<Tag>, Error>;

    /// Creates a tag on the given commit.
    fn create_tag(&mut self, tag: &Tag, commit_hash: &CommitHash) -> Result<(), Error>;

    /// Gets the commit that the tag points to.
    fn locate_tag(&self, tag: &Tag) -> Result<CommitHash, Error>;

    /// Gets the tags on the given commit.
    fn get_tag(&self, commit_hash: &CommitHash) -> Result<Vec<Tag>, Error>;

    /// Removes the tag.
    fn remove_tag(&mut self, tag: &Tag) -> Result<(), Error>;

    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Creates a commit from the currently checked out branch.
    fn create_commit(
        &mut self,
        commit_message: &str,
        diff: Option<&str>,
    ) -> Result<CommitHash, Error>;

    /// Creates a semantic commit from the currently checked out branch.
    fn create_semantic_commit(&mut self, commit: SemanticCommit)
        -> Result<CommitHash, Error>;

    /// Reads the reserved state from the current working tree.
    fn read_semantic_commit(&self, commit_hash: &CommitHash)
        -> Result<SemanticCommit, Error>;

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    fn run_garbage_collection(&mut self) -> Result<(), Error>;

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    fn checkout_clean(&mut self) -> Result<(), Error>;

    /// Checkouts to the branch.
    fn checkout(&mut self, branch: &Branch) -> Result<(), Error>;

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    fn checkout_detach(&mut self, commit_hash: &CommitHash) -> Result<(), Error>;

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    fn get_head(&self) -> Result<CommitHash, Error>;

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    fn get_initial_commit(&self) -> Result<CommitHash, Error>;

    /// Returns the diff of the given commit.
    fn show_commit(&self, commit_hash: &CommitHash) -> Result<String, Error>;

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    /// * `max`: the maximum number of entries to be returned.
    fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    /// * `max`: the maximum number of entries to be returned.
    fn list_descendants(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Returns the children commits of the given commit.
    fn list_children(&self, commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error>;

    /// Returns the merge base of the two commits.
    fn find_merge_base(
        &self,
        commit_hash1: &CommitHash,
        commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error>;

    // ----------------------------
    // Remote-related methods
    // ----------------------------

    /// Adds a remote repository.
    fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error>;

    /// Removes a remote repository.
    fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error>;

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    fn fetch_all(&mut self) -> Result<(), Error>;

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    fn list_remotes(&self) -> Result<Vec<(String, String)>, Error>;

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, remote_url, commit_hash)`
    fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error>;
}

pub struct CurRepository {
    repo: Repository
}

//TODO: error handling, error name
#[async_trait]
impl RawRepository for CurRepository{
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
    fn init(directory: &str) -> Result<Self, Error>
    where
        Self: Sized {
            match Repository::open(directory) {
                Ok(_repo) => Err(Error::InvalidRepository("There is an already existing repository".to_string())),
                Err(_e) => {
                    let repo = Repository::init(directory)
                        .map_err(|e| Error::from(e))?;
                    Ok(Self{ repo })
            }   
        }
    }

    // Loads an exisitng repository.
    fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized {
            let repo = Repository::open(directory).map_err(|e| Error::from(e))?;
            Ok(Self{ repo })
        }

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let branches = self.repo.branches(Option::Some(BranchType::Local))
            .map_err(|e| Error::from(e))?;

        let branch_name_list = branches.map(|branch| {
            let branch_name = branch.map_err(|e| Error::from(e))?
                .0.name()
                .map_err(|e| Error::from(e))?
                .map(|name| name.to_string())
                .ok_or(Error::Unknown("err".to_string()))?;

            Ok(branch_name)
        }).collect::<Result<Vec<Branch>, Error>>();

        branch_name_list
    }

    /// Creates a branch on the commit.
    fn create_branch(
        &self,
        branch_name: &Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error>{
        let oid = Oid::from_bytes(&commit_hash.hash).map_err(|e| Error::from(e))?;
        let commit = self.repo.find_commit(oid)
            .map_err(|e| Error::from(e))?;
        
        //if force true and branch already exists, it replaces with new one
        let _branch = self.repo.branch(
            branch_name.as_str(),
            &commit,
            false
            ).map_err(|e| Error::from(e))?;
        
        Ok(())
    }

    /// Gets the commit that the branch points to.
    fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error>{
        let branch = self.repo.find_branch(
            branch, 
            BranchType::Local
        ).map_err(|e| Error::from(e))?;
        let oid = branch.get().target()
            .ok_or(Error::Unknown("err".to_string()))?; //TODO: error message
        let hash = <[u8; 20]>::try_from(oid.as_bytes())
            .map_err(|_| Error::Unknown("err".to_string()))?; //TODO: error message
        
        Ok(CommitHash{ hash })
    }

    //TODO: if commit just is part of branch..? not directly pointed
    /// Gets the list of branches from the commit.
    fn get_branches(&self, commit_hash: &CommitHash) -> Result<Vec<Branch>, Error>{
        unimplemented!()
        /*
        let branches = self.repo.branches(
            Option::Some(BranchType::Local)
        ).map_err(|e| Error::from(e))?;

        let branch_name_list = branches.into_iter().filter_map(|b| 
b
                .map_err(|&e| Error::from(e))?
                .0.get().target()
                .ok_or(Error::Unknown("err".to_string()))? 
                == git2::Oid::from_bytes(&commit_hash.hash)
                .map_err(|e| Error::from(e))?
        ).collect::<Result<Vec<(git2::Branch, BranchType)>, git2::Error>>()?.iter()
        .map(|&branch| {
            let name = branch.0.name()
                .map_err(|e| Error::from(e))?
                .map(|name| name.to_string())
                .ok_or(Error::Unknown("err".to_string()))?;

            Ok(name)
        }).collect::<Result<Vec<Branch>, Error>>();

        branch_name_list */
    }

    /// Moves the branch.
    fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash)
        -> Result<(), Error>{
            let mut git2_branch = self.repo.find_branch(
                branch, 
                BranchType::Local
            ).map_err(|e| Error::from(e))?;
            let oid = Oid::from_bytes(&commit_hash.hash)
                .map_err(|e| Error::from(e))?;
            let reflog_msg = ""; //TODO: reflog_msg
            let mut reference = git2_branch.get_mut();
            let _set_branch = git2::Reference::set_target(&mut reference, oid, reflog_msg)
                .map_err(|e| Error::from(e));

            Ok(())
        }

    /// Deletes the branch.
    fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error>{
        let mut git2_branch = self.repo.find_branch(
            branch, 
            BranchType::Local
        ).map_err(|e| Error::from(e))?;
        
        let current_branch = self.repo.head()
            .map_err(|e| Error::from(e))?
            .shorthand()
            .ok_or(Error::Unknown("err".to_string()))?
            .to_string();
        
        let res = if &current_branch == branch {
            Err(Error::InvalidRepository(("Given branch is currently checkout branch").to_string()))
        }else{
            git2_branch.delete().map_err(|e| Error::from(e))
        };
        println!("{:?}", res);
        res
    }

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    fn list_tags(&self) -> Result<Vec<Tag>, Error>{
        //pattern defines what tags you want to get
        let tag_array=  self.repo.tag_names( None)
            .map_err(|e| Error::from(e))?;

        let tag_list = tag_array.iter().map(|tag| {
            let tag_name = tag.ok_or(Error::Unknown("err".to_string()))?.to_string();

            Ok(tag_name)
        }).collect::<Result<Vec<Tag>, Error>>();

        tag_list
    }

    /// Creates a tag on the given commit.
    fn create_tag(&mut self, tag: &Tag, commit_hash: &CommitHash) -> Result<(), Error>{
        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;

        let object = self.repo.find_object(
            oid, 
            Some(ObjectType::Commit)
        ).map_err(|e| Error::from(e))?;
        let tagger = self.repo.signature()
            .map_err(|e| Error::from(e))?;
        let tag_message = ""; //TODO: tag_message

        //if force true and tag already exists, it replaces with new one
        let _tag = self.repo.tag(
            tag.as_str(), 
            &object, 
            &tagger, 
            tag_message, 
            false
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    //TODO: unwrap()
    /// Gets the commit that the tag points to.
    fn locate_tag(&self, tag: &Tag) -> Result<CommitHash, Error>{
        let reference = self.repo.find_reference(
            &("refs/tags/".to_owned() + tag) //TODO: right?
        ).map_err(|e| Error::from(e))?;

        let object = reference.peel(ObjectType::Commit)
            .map_err(|e| Error::from(e))?;
        
        let oid = object.id();
        let hash = <[u8; 20]>::try_from(oid.as_bytes())
            .map_err(|e| Error::Unknown("abc".to_string()))?; //TODO: error message
        let commit_hash = CommitHash{ hash }; 
        Ok(commit_hash)
    }

    //TODO: unwrap()
    /// Gets the tags on the given commit.
    fn get_tag(&self, commit_hash: &CommitHash) -> Result<Vec<Tag>, Error>{
        unimplemented!()
        
               // let references_tag = self.repo.references_glob("refs/tags/*")
           // .map_err(|e| Error::from(e))?;

        
        /*
        //tags from one commit
        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;

        let references = self.repo.references()
            .map_err(|e| Error::from(e))?;
        
        let refs = references.filter(|reference| 
            reference.as_ref().unwrap().is_tag()
        ).collect::<Vec<Result<Reference, git2::Error>>>();

        let tags = refs.iter().map(|x| 
            x.unwrap().peel_to_tag().unwrap()
        ).collect::<Vec<git2::Tag>>();

        let tags_filter = tags.into_iter().filter(|target| 
          target.target().unwrap().id() == oid
        ).collect::<Vec<git2::Tag>>(); //TODO: type
        //Repository::find_tag()
        let a = [1, 2, 3];

        let doubled = a.into_iter().filter(|&x| x ==2 ).collect::<Vec<i32>>();

        let res = tags_filter.iter().map(|target| target.name().unwrap().to_string())
        .collect::<Vec<Tag>>();
        Ok(res)*/
        
    }

    /// Removes the tag.
    fn remove_tag(&mut self, tag: &Tag) -> Result<(), Error>{
        self.repo.tag_delete(tag.as_str()).map_err(|e| Error::from(e))
    }
    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Create a commit from the currently checked out branch.
    fn create_commit(
        &mut self,
        commit_message: &str,
        diff: Option<&str>,
    ) -> Result<CommitHash, Error>{
        unimplemented!()
        /*
        //get current branch
        let head = self.repo.head().unwrap();
        if !head.is_branch(){
            //TODO: Err
        }
        //TODO: should check head(reference) is same as branch
        //needs filename to make file 
    
        //get branch: head->reference->oid->commit->branch or head->reference->object::commit->branch (use peel)
        let mut index = self.repo.index().unwrap(); //index == staging area, get index file
        let p = Path::new(self.repo.workdir().unwrap()).join("TODO: file name?"); //workding directory path
        println!("using path {:?}", p);
        fs::File::create(&p).unwrap(); //make file in the working directory
        index.add_path(Path::new("TODO: file name?")).unwrap(); //update index entry with the file path in the directory which is relative to working directory
        let oid_tree = index.write_tree().unwrap(); //make tree of that index file
        let tree = self.repo.find_tree(oid_tree).unwrap();

        let sig = self.repo.signature().unwrap();
        let parent_commit_hash = self.locate_branch(branch); //TODO: find branch
        let parent_commit = Repository::find_commit(&self.repo, git2::Oid::from_bytes(&parent_commit_hash.hash).unwrap()).unwrap();
        
        let oid_new = self.repo
            .commit(
                Some(&("refs/heads/".to_owned() + branch)), //TODO: &Branch -> Branch or just replace with head->name
                &sig,
                &sig,
                commit_message,
                &tree,
                &[&parent_commit],
            )
            .unwrap();
        let commit_new = self.repo.find_commit(oid_new).unwrap();

        //TODO: does it need to clear index?
        self.repo.reset(commit_new.as_object(), git2::ResetType::Soft, None);*/

    }


    /// Creates a semantic commit from the currently checked out branch.
    fn create_semantic_commit(&mut self, commit: SemanticCommit)
        -> Result<CommitHash, Error>{
         /*   pub title: String,
            pub body: String,
            /// (If this commit made any change) the new reserved state.
            pub reserved_state: Option<ReservedState>,*/
        
            //commit message만 달라짐
            unimplemented!()    
        }

    /// Reads the reserved state from the current working tree.
    fn read_semantic_commit(&self, commit_hash: &CommitHash)
        -> Result<SemanticCommit, Error>{
            unimplemented!()
        }

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    fn run_garbage_collection(&mut self) -> Result<(), Error>{
        unimplemented!()
        //TODO: check all of the references and identify orphaned commits
    }

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    fn checkout_clean(&mut self) -> Result<(), Error>{
        unimplemented!()
        //reset unstaged files and remove untracked files including directory
        //TODO: check repo.statues() and statusOption
    }

    /// Checkouts to the branch.
    fn checkout(&mut self, branch: &Branch) -> Result<(), Error>{
        let obj = self.repo.revparse_single(
            &("refs/heads/".to_owned() + branch)
        ).map_err(|e| Error::from(e))?;

        self.repo.checkout_tree(
            &obj,
            None
        ).map_err(|e| Error::from(e));

        self.repo.set_head(
            &("refs/heads/".to_owned() + branch)
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    fn checkout_detach(&mut self, commit_hash: &CommitHash) -> Result<(), Error>{
        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;

        self.repo.set_head_detached(oid)
            .map_err(|e| Error::from(e));

        Ok(())
        //https://stackoverflow.com/questions/55141013/how-to-get-the-behaviour-of-git-checkout-in-rust-git2
    }

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    fn get_head(&self) -> Result<CommitHash, Error>{
        let ref_head = self.repo.head()
            .map_err(|e| Error::from(e))?;
        let oid = ref_head.target()
            .ok_or(Error::Unknown("err".to_string()))?;
        let hash = <[u8; 20]>::try_from(oid.as_bytes())
            .map_err(|_| Error::Unknown("abc".to_string()))?; //TODO: error message
    
        Ok(CommitHash{ hash })
    }

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    fn get_initial_commit(&self) -> Result<CommitHash, Error>{
        //check if the repsotiroy is empty
        //TODO: is this right?
        
        let _head = self.repo.head()
            .map_err(|_| Error::InvalidRepository("Repository is empty".to_string()))?;

        //TODO: A revwalk allows traversal of the commit graph defined by including one or
        //      more leaves and excluding one or more roots.
        //      --> revwalk can make error if there exists one or more roots...
        //if not
        let mut revwalk = self.repo.revwalk()?;

        revwalk.push_head()
            .map_err(|e| Error::from(e))?;
        revwalk.set_sorting(
            git2::Sort::TIME | git2::Sort::REVERSE
        );

        let oids: Vec<Oid> = revwalk.by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()
            .map_err(|e| Error::from(e))?; //TODO: is this right?

        //TODO: what if oids[0] not exist?
        let hash = <[u8; 20]>::try_from(oids[0].as_bytes()).map_err(|_| Error::Unknown("abc".to_string()))?; //TODO: error message
        
        Ok(CommitHash{ hash }) //TODO: oid -> CommitHash

        //https://users.rust-lang.org/t/make-sure-git2-revwalk-is-linear/25560/3
    }

    /// Returns the diff of the given commit.
    fn show_commit(&self, commit_hash: &CommitHash) -> Result<String, Error>{
        unimplemented!()
        //Diff: tree_to_tree
        //https://stackoverflow.com/questions/68170627/how-to-get-the-behavior-of-git-diff-master-commitdirectory-in-rust-git2

        //TODO: get previous commit and get tree..?/blob and compare..?
        //should search about git2::Diff
        //git2::Diff

    }

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    /// * `max`: the maximum number of entries to be returned.
    fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>{
        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;
        let mut revwalk = self.repo.revwalk()?;

        revwalk.push(oid)
            .map_err(|e| Error::from(e))?;
        revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL); //TODO: should be tested

        //compare max and ancestor's size
        let oids: Vec<Oid> = revwalk.by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()
            .map_err(|e| Error::from(e))?; //TODO: is this right?
        
        let oids = oids[1..oids.len()].to_vec();

        let oids_ancestor = if let Some(num_max) = max{
            for n in 0..num_max {
                //TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oids[n])
                    .map_err(|e| Error::from(e))?;
                let num_parents = commit.parents().len();
                
                if num_parents > 1 {
                    return Err(Error::InvalidRepository("There exists a merge commit".to_string()));
                }
                //TODO: should check current commit's parent == oids[next]
            }
            oids[0..num_max].to_vec()
        }else{ //if max==None
            let mut i = 0;
            
            loop{
                //TODO: Check first one should be commit_hash
                let commit = self.repo.find_commit(oids[i])
                    .map_err(|e| Error::from(e))?;
                let num_parents = commit.parents().len();
                
                if num_parents > 1 {
                    return Err(Error::InvalidRepository("There exists a merge commit".to_string()));
                }
                //TODO: should check current commit's parent == oids[next]
                if num_parents == 0{
                    break;
                }
                i = i + 1;
            }
            oids
        };

        let ancestors = oids_ancestor.iter().map(|&oid|{
            let hash: [u8; 20] = oid.as_bytes().try_into()
                .map_err(|_| Error::Unknown("abc".to_string()))?; 
            Ok(CommitHash{ hash })
        }).collect::<Result<Vec<CommitHash>, Error>>();

        ancestors
    }

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    /// * `max`: the maximum number of entries to be returned.
    fn list_descendants(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>{
        unimplemented!()
    }

    /// Returns the children commits of the given commit.
    fn list_children(&self, commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error>{
        unimplemented!()
    }

    /// Returns the merge base of the two commits.
    fn find_merge_base(
        &self,
        commit_hash1: &CommitHash,
        commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error>{
        let oid1 = Oid::from_bytes(&commit_hash1.hash).map_err(|e| Error::from(e))?;
        let oid2 = Oid::from_bytes(&commit_hash2.hash).map_err(|e| Error::from(e))?;

        let oid_merge = self.repo.merge_base(oid1, oid2)
            .map_err(|e| Error::from(e))?;
        let commit_hash_merge: [u8; 20] = oid_merge.as_bytes().try_into()
            .map_err(|_| Error::Unknown("abc".to_string()))?; 

        Ok(CommitHash{hash: commit_hash_merge})
    }

    // ----------------------------
    // Remote-related methods
    // ----------------------------

    /// Adds a remote repository.
    fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error>{
        let _remote = self.repo.remote(
            remote_name, 
            remote_url
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    /// Removes a remote repository.
    fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error>{
        let _remote_delete = self.repo.remote_delete(
            remote_name
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    fn fetch_all(&mut self) -> Result<(), Error>{
        //1. get all of remote repository name and its branches which are used below
        //git fetch origin/main == repo.find_remote("origin")?.fetch(&["main"], None, None)
        //TODO: &["*"] works? or should find (remote, branch) ...
        unimplemented!()
    }

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    fn list_remotes(&self) -> Result<Vec<(String, String)>, Error>{
        let remote_array = self.repo.remotes()
            .map_err(|e| Error::from(e))?;

        let remote_name_list = remote_array.iter().map(|remote| {
            let remote_name = remote
                .ok_or_else(|| Error::Unknown("unable to get remote".to_string()))?
                .to_string();
            
            Ok(remote_name)
        }).collect::<Result<Vec<String>, Error>>()?;

        let res = remote_name_list.iter().map(|name|{
            let remote = self.repo.find_remote(
                name.clone().as_str() 
            ).map_err(|e| Error::from(e))?;

            let url = remote.url()
                .ok_or_else(|| Error::Unknown("unable to get valid url".to_string()))?;

            Ok((name.clone(), url.to_string()))
        }).collect::<Result<Vec<(String, String)>, Error>>();

        res
    }

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, remote_url, commit_hash)`
    fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error>{
        unimplemented!()
        //TODO: remote_name - branch ??
        //1. get (remote_name, remote_url) from list_remotes
        //2. can get commit object from rev_single but don't know what remote contains what branches
        //branches by type remote can get remote branches but don't know each branches' remote name
    }
}

#[cfg(test)]
mod tests {
    use git2::{Repository, RepositoryInitOptions, Oid, };
    use std::path::Path;
    use tempfile::TempDir;
    use crate::raw::{RawRepository, CurRepository};
    use crate::CommitHash;

    //make a repository which includes one initial commit at "main" branch
    //this returns CurRepository containing the repository
    fn init_repository_with_initial_commit(path: &Path) -> CurRepository {
        let mut opts = RepositoryInitOptions::new();
        opts.initial_head("main");
        let repo = Repository::init_opts(path, &opts).unwrap();

        //make an initial commit and set it "HEAD"
        let oid: Oid;
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "name").unwrap();
            config.set_str("user.email", "email").unwrap();
            let mut index = repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = repo.find_tree(id).unwrap();
            let sig = repo.signature().unwrap();
            
            oid = repo.commit(
                Some("HEAD"), 
                &sig, 
                &sig, 
                "initial\nbody", 
                &tree, 
                &[]
            ).unwrap();
        }
        
        //create branch "main" at the initial commit
        let cur_repo = CurRepository{ repo };
        let hash = <[u8; 20]>::try_from(oid.as_bytes()).unwrap();
        cur_repo.create_branch(&("main".to_owned()), CommitHash{ hash });
    
        cur_repo
    }

    //initialize repository with empty commit and empty branch
    #[test]
    fn init() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let cur_repo = CurRepository::init(path.to_str().unwrap()).unwrap();

        assert!(!cur_repo.repo.is_bare());
        assert!(cur_repo.repo.is_empty().unwrap());
    }

    //open existed repository and verifies whether it opens well
    #[test]
    fn open() {
        let td = TempDir::new().unwrap();
        let path = td.path(); 

        let init_repo= init_repository_with_initial_commit(path);
        let open_repo = CurRepository::open(path.to_str().unwrap()).unwrap();

        assert!(!open_repo.repo.is_bare());
        assert!(!open_repo.repo.is_empty().unwrap());

        let branch_list_init = init_repo.list_branches().unwrap();
        let branch_list_open = open_repo.list_branches().unwrap();
        
        assert_eq!(branch_list_init.len(), branch_list_open.len());
        assert_eq!(branch_list_init[0], branch_list_open[0]);
    }

    /*
        c2 (HEAD -> main)
         |
        c1 (branch_1)
     */
    //create "branch_1" at c1, create c2 at "main" branch, move "branch_1" head from c1 to c2
    //finally, "branch_1" is removed
    #[test]
    fn branch(){
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut cur_repo= init_repository_with_initial_commit(path);

        //there is one branch "main" at initial state
        let mut branch_list = cur_repo.list_branches().unwrap();
        assert_eq!(branch_list.len(), 1);
        assert_eq!(branch_list[0], "main".to_owned());

        //git branch branch_1
        let head = cur_repo.get_head().unwrap();
        cur_repo.create_branch(&("branch_1".to_owned()), head);
        branch_list = cur_repo.list_branches().unwrap();

        //branch_list is sorted by branch names' alphabetic order
        assert_eq!(branch_list.len(), 2);
        assert_eq!(branch_list[0], "branch_1".to_owned());
        assert_eq!(branch_list[1], "main".to_owned());

        let branch_1_commit_hash = cur_repo.locate_branch(&("branch_1".to_owned())).unwrap();
        assert_eq!(branch_1_commit_hash, head);

        //make second commit with "main" branch
        {
            let head_oid = cur_repo.repo.head().unwrap().target().unwrap();
            let head_commit = cur_repo.repo.find_commit(head_oid).unwrap();

            let mut index = cur_repo.repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = cur_repo.repo.find_tree(id).unwrap();
            let sig = cur_repo.repo.signature().unwrap();
            cur_repo.repo.commit(
                Some("HEAD"), 
                &sig, 
                &sig, 
                "second", 
                &tree, 
                &[&head_commit]
            ).unwrap();
        }
        //move "branch_1" head to "main" head
        let main_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();
        cur_repo.move_branch(
            &("branch_1".to_owned()), 
            &main_commit_hash
        );
        let branch_1_commit_hash = cur_repo.locate_branch(&("branch_1".to_owned())).unwrap();
        assert_eq!(main_commit_hash, branch_1_commit_hash);
    
        //remove "branch_1" and the remaining branch should be only "main"
        cur_repo.delete_branch(&("branch_1".to_owned()));
        let branch_list = cur_repo.list_branches().unwrap();
        assert_eq!(branch_list.len(), 1);
        assert_eq!(branch_list[0], "main".to_owned());

        //TODO:
        let remove_main = cur_repo.delete_branch(&("main".to_owned()));
        let res = match remove_main{
            Ok(_) => "success".to_owned(),
            Err(_) => "failure".to_owned()
        };
        assert_eq!(res, "failure".to_owned());
    }

    //create a tag and remove it
    #[test]
    fn tag(){
        let td = TempDir::new().unwrap();
        let path = td.path(); 
        let mut cur_repo= init_repository_with_initial_commit(path);

        //there is no tags at initial state
        let tag_list = cur_repo.list_tags().unwrap();
        assert_eq!(tag_list.len(), 0);

        //create "tag_1" at first commit
        let first_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();
        cur_repo.create_tag(
            &("tag_1".to_owned()), 
            &first_commit_hash
        );
        let tag_list = cur_repo.list_tags().unwrap();
        assert_eq!(tag_list.len(), 1);
        assert_eq!(tag_list[0], "tag_1".to_owned());

        let tag_1_commit_hash = cur_repo.locate_tag(&("tag_1".to_owned())).unwrap();
        assert_eq!(first_commit_hash, tag_1_commit_hash);

        //remove "tag_1"
        cur_repo.remove_tag(&("tag_1".to_owned()));
        let tag_list = cur_repo.list_tags().unwrap();
        assert_eq!(tag_list.len(), 0);
    }

    #[test]
    fn checkout(){
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut cur_repo= init_repository_with_initial_commit(path);

        {
            let first_oid = cur_repo.repo.head().unwrap().target().unwrap();
            let first_commit = cur_repo.repo.find_commit(first_oid).unwrap();
            let first_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();
            cur_repo.create_branch(&("branch_1".to_owned()), first_commit_hash);

            //make second commit at "main" branch
            let mut index = cur_repo.repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = cur_repo.repo.find_tree(id).unwrap();
            let sig = cur_repo.repo.signature().unwrap();

            let second_oid = cur_repo.repo.commit(
                Some("HEAD"), 
                &sig, 
                &sig, 
                "second", 
                &tree, 
                &[&first_commit]
            ).unwrap();
            let second_commit = cur_repo.repo.find_commit(second_oid).unwrap();
            let second_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();
            cur_repo.create_branch(&("branch_2".to_owned()), second_commit_hash);

            //make third commit at "main" branch
            let mut index = cur_repo.repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = cur_repo.repo.find_tree(id).unwrap();
            let sig = cur_repo.repo.signature().unwrap();
            let _third_oid = cur_repo.repo.commit(
                Some("HEAD"), 
                &sig, 
                &sig, 
                "third", 
                &tree, 
                &[&second_commit]
            ).unwrap();
        }
        let first_commit_hash = cur_repo.locate_branch(&("branch_1".to_owned())).unwrap();
        let second_commit_hash = cur_repo.locate_branch(&("branch_2".to_owned())).unwrap();
        let third_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();

        cur_repo.checkout(&("branch_1".to_owned()));
        let head_commit_hash = cur_repo.get_head().unwrap();
        assert_eq!(head_commit_hash, first_commit_hash);
        cur_repo.checkout(&("branch_2".to_owned()));
        let head_commit_hash = cur_repo.get_head().unwrap();
        assert_eq!(head_commit_hash, second_commit_hash);
        cur_repo.checkout(&("main".to_owned()));
        let head_commit_hash = cur_repo.get_head().unwrap();
        assert_eq!(head_commit_hash, third_commit_hash);

    }

    
    /*
        c2 (HEAD -> main)       c2 (main)
         |                 -->   |
        c1 (branch_1)           c1 (HEAD)
    */
    //checkout to commit and set "HEAD" to the detached mode
    #[test]
    fn checkout_detach(){
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut cur_repo= init_repository_with_initial_commit(path);

        //there is one branch "main" at initial state
        let mut branch_list = cur_repo.list_branches().unwrap();
        assert_eq!(branch_list.len(), 1);
        assert_eq!(branch_list[0], "main".to_owned());

        let commit1 = cur_repo.get_head().unwrap();        
        //make second commit with "main" branch
        {
            let head_oid = cur_repo.repo.head().unwrap().target().unwrap();
            let head_commit = cur_repo.repo.find_commit(head_oid).unwrap();

            let mut index = cur_repo.repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = cur_repo.repo.find_tree(id).unwrap();
            let sig = cur_repo.repo.signature().unwrap();
            cur_repo.repo.commit(                                                                                
                Some("HEAD"), 
                &sig, 
                &sig, 
                "second", 
                &tree, 
                &[&head_commit]
            ).unwrap();
        }
        //checkout to commit1 and set HEAD detached mode
        cur_repo.checkout_detach(&commit1);
        let cur_head_name = cur_repo.repo.head().unwrap().name().unwrap().to_string();
        let cur_head_commit_hash = cur_repo.get_head().unwrap();

        //this means the current head is at a detached mode,
        //otherwise this should be "refs/heads/main"
        assert_eq!(cur_head_name, "HEAD");
        assert_eq!(cur_head_commit_hash, commit1);
    }

    /*  
        c3 (HEAD -> main)
        |  
        c2
        | 
        c1 
    */
    //get ancestors of c3 which are [c2, c1] in the linear commit above
    #[test]
    fn ancestor(){
        let td = TempDir::new().unwrap();
        let path = td.path();
        let cur_repo= init_repository_with_initial_commit(path);
        
        let first_oid = cur_repo.repo.head().unwrap().target().unwrap();
        let first_commit = cur_repo.repo.find_commit(first_oid).unwrap();
        let first_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();

        //make second commit at "main" branch
        let mut index = cur_repo.repo.index().unwrap();
        let id = index.write_tree().unwrap();

        let tree = cur_repo.repo.find_tree(id).unwrap();
        let sig = cur_repo.repo.signature().unwrap();

        let second_oid = cur_repo.repo.commit(
            Some("HEAD"), 
            &sig, 
            &sig, 
            "second", 
            &tree, 
            &[&first_commit]
        ).unwrap();
        let second_commit = cur_repo.repo.find_commit(second_oid).unwrap();
        let second_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();

        //make third commit at "main" branch
        let mut index = cur_repo.repo.index().unwrap();
        let id = index.write_tree().unwrap();

        let tree = cur_repo.repo.find_tree(id).unwrap();
        let sig = cur_repo.repo.signature().unwrap();
        let _third_oid = cur_repo.repo.commit(
            Some("HEAD"), 
            &sig, 
            &sig, 
            "third", 
            &tree, 
            &[&second_commit]
        ).unwrap();
        
        let third_commit_hash = cur_repo.locate_branch(&("main".to_owned())).unwrap();

        //get only one ancestor(direct parent)
        let ancestors = cur_repo.list_ancestors(
            &third_commit_hash, 
            Some(1)
        ).unwrap();
        assert_eq!(ancestors.len(), 1);
        assert_eq!(ancestors[0], second_commit_hash);

        //get two ancestors with max 2
        let ancestors = cur_repo.list_ancestors(
            &third_commit_hash, 
            Some(2)
        ).unwrap();
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0], second_commit_hash);
        assert_eq!(ancestors[1], first_commit_hash);

        //get all ancestors
        let ancestors = cur_repo.list_ancestors(
            &third_commit_hash, 
            None
        ).unwrap();
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0], second_commit_hash);
        assert_eq!(ancestors[1], first_commit_hash);

        //TODO: if max num > the number of ancestors
    }

    /*
        c3 (HEAD -> branch_b)
         |  c2 (branch_a)
         | /
        c1 (main)
    */
    //make three commits at different branches and the merge base of (c2,c3) would be c1
    #[test]
    fn merge_base() {
        let td = TempDir::new().unwrap();
        let path = td.path();
        let mut cur_repo= init_repository_with_initial_commit(path);

        //create three branches at c1
        {
            let commit_hash1 = cur_repo.locate_branch(&("main".to_owned())).unwrap();
            cur_repo.create_branch(
                &("branch_a".to_owned()), 
                commit_hash1
            ).unwrap();
            cur_repo.create_branch(
                &("branch_b".to_owned()), 
                commit_hash1
            ).unwrap();
        }
        //make a commit at "branch_a" branch
        {
            cur_repo.checkout(&("branch_a".to_owned()));
            let oid1 = cur_repo.repo.head().unwrap().target().unwrap();
            let commit1 = cur_repo.repo.find_commit(oid1).unwrap();

            let mut index = cur_repo.repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = cur_repo.repo.find_tree(id).unwrap();
            let sig = cur_repo.repo.signature().unwrap();
            cur_repo.repo.commit(
                Some("refs/heads/branch_a"), 
                &sig, 
                &sig, 
                "branch_a", 
                &tree, 
                &[&commit1]
            ).unwrap();
        }
        //make a commit at "branch_b" branch
        {
            cur_repo.checkout(&("branch_b".to_owned()));
            let oid1 = cur_repo.repo.head().unwrap().target().unwrap();
            let commit1 = cur_repo.repo.find_commit(oid1).unwrap();

            let mut index = cur_repo.repo.index().unwrap();
            let id = index.write_tree().unwrap();

            let tree = cur_repo.repo.find_tree(id).unwrap();
            let sig = cur_repo.repo.signature().unwrap();
            cur_repo.repo.commit(
                Some("refs/heads/branch_b"), 
                &sig, 
                &sig, 
                "branch_b", 
                &tree, 
                &[&commit1]
            ).unwrap();
        }
        //make merge base of (c2,c3)
        let commit_hash1 = cur_repo.locate_branch(&("main".to_owned())).unwrap();
        let commit_hash_a = cur_repo.locate_branch(&("branch_a".to_owned())).unwrap();
        let commit_hash_b = cur_repo.locate_branch(&("branch_b".to_owned())).unwrap();
        let merge_base = cur_repo.find_merge_base(
            &commit_hash_a, 
            &commit_hash_b
        ).unwrap();

        //the merge base of (c2,c3) should be c1
        assert_eq!(merge_base, commit_hash1);
    }

    //add remote repository and remove it
    #[test]
    fn remote(){
        let td = TempDir::new().unwrap();
        let path = td.path(); 
        let mut cur_repo= init_repository_with_initial_commit(path);

        //add dummy remote
        cur_repo.add_remote("origin", "/path/to/nowhere");

        let remote_list = cur_repo.list_remotes().unwrap();
        assert_eq!(remote_list.len(), 1);
        assert_eq!(remote_list[0].0, "origin".to_owned());
        assert_eq!(remote_list[0].1, "/path/to/nowhere".to_owned());
        {
            let origin = cur_repo.repo.find_remote("origin").unwrap();
            assert_eq!(origin.name(), Some("origin"));
            assert_eq!(origin.url(), Some("/path/to/nowhere"));
            assert_eq!(origin.pushurl(), None);
        }
        //remove dummy remote
        cur_repo.remove_remote("origin");
        let remote_list = cur_repo.list_remotes().unwrap();
        assert_eq!(remote_list.len(), 0);
    }
}