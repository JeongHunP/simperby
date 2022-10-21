use super::*;
use async_trait::async_trait;
use simperby_common::reserved::ReservedState;
use thiserror::Error;
use git2::{Repository, BranchType, Oid, ObjectType, Reference};//Branch;
use tokio::sync::RwLock;
use std::path::Path;
use std::fs::{self};
use std::sync::Arc;

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
    async fn init(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    // Loads an exisitng repository.
    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized;

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    async fn list_branches(&self) -> Result<Vec<Branch>, Error>;

    /// Creates a branch on the commit.
    async fn create_branch(
        &self,
        branch_name: &Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error>;

    /// Gets the commit that the branch points to.
    async fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error>;

    /// Gets the list of branches from the commit.
    async fn get_branches(&self, commit_hash: &CommitHash) -> Result<Vec<Branch>, Error>;

    /// Moves the branch.
    async fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash)
        -> Result<(), Error>;

    /// Deletes the branch.
    async fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error>;

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    async fn list_tags(&self) -> Result<Vec<Tag>, Error>;

    /// Creates a tag on the given commit.
    async fn create_tag(&mut self, tag: &Tag, commit_hash: &CommitHash) -> Result<(), Error>;

    /// Gets the commit that the tag points to.
    async fn locate_tag(&self, tag: &Tag) -> Result<CommitHash, Error>;

    /// Gets the tags on the given commit.
    async fn get_tag(&self, commit_hash: &CommitHash) -> Result<Vec<Tag>, Error>;

    /// Removes the tag.
    async fn remove_tag(&mut self, tag: &Tag) -> Result<(), Error>;

    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Creates a commit from the currently checked out branch.
    async fn create_commit(
        &mut self,
        commit_message: &str,
        diff: Option<&str>,
    ) -> Result<CommitHash, Error>;

    /// Creates a semantic commit from the currently checked out branch.
    async fn create_semantic_commit(&mut self, commit: SemanticCommit)
        -> Result<CommitHash, Error>;

    /// Reads the reserved state from the current working tree.
    async fn read_semantic_commit(&self, commit_hash: &CommitHash)
        -> Result<SemanticCommit, Error>;

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    async fn run_garbage_collection(&mut self) -> Result<(), Error>;

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    async fn checkout_clean(&mut self) -> Result<(), Error>;

    /// Checkouts to the branch.
    async fn checkout(&mut self, branch: &Branch) -> Result<(), Error>;

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    async fn checkout_detach(&mut self, commit_hash: &CommitHash) -> Result<(), Error>;

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    async fn get_head(&self) -> Result<CommitHash, Error>;

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    async fn get_initial_commit(&self) -> Result<CommitHash, Error>;

    /// Returns the diff of the given commit.
    async fn show_commit(&self, commit_hash: &CommitHash) -> Result<String, Error>;

    /// Lists the ancestor commits of the given commit (The first element is the direct parent).
    ///
    /// It fails if there is a merge commit.
    /// * `max`: the maximum number of entries to be returned.
    async fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    /// * `max`: the maximum number of entries to be returned.
    async fn list_descendants(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>;

    /// Returns the children commits of the given commit.
    async fn list_children(&self, commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error>;

    /// Returns the merge base of the two commits.
    async fn find_merge_base(
        &self,
        commit_hash1: &CommitHash,
        commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error>;

    // ----------------------------
    // Remote-related methods
    // ----------------------------

    /// Adds a remote repository.
    async fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error>;

    /// Removes a remote repository.
    async fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error>;

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    async fn fetch_all(&mut self) -> Result<(), Error>;

    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error>;

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, remote_url, commit_hash)`
    async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error>;
}

pub struct CurRepository {
    repo: Arc<tokio::sync::RwLock<Repository>>
    //directory: String
}

//TODO:
//1. oid->CommitHash
//2. ok_or
//3. Repository:: -> self.repo.
//4. Send issue
#[async_trait]
impl RawRepository for CurRepository{
    /// Initialize the genesis repository from the genesis working tree.
    ///
    /// Fails if there is already a repository.
    async fn init(directory: &str) -> Result<Self, Error>
    where
        Self: Sized {
            match Repository::open(directory) {
                Ok(repo) => Err(Error::InvalidRepository("There is an already existing repository".to_string())),
                Err(e) => {
                    let repo = Repository::init(directory).map_err(|e| Error::from(e))?;
                    let repo_lock = Arc::new(RwLock::new(repo));
                    Ok(CurRepository{ repo: repo_lock })
            }   
        }
    }

    // Loads an exisitng repository.
    async fn open(directory: &str) -> Result<Self, Error>
    where
        Self: Sized {
            let repo = Repository::open(directory).map_err(|e| Error::from(e))?;
            Ok(CurRepository{ repo })
        }

    // ----------------------
    // Branch-related methods
    // ----------------------

    /// Returns the list of branches.
    async fn list_branches(&self) -> Result<Vec<Branch>, Error> {
        let branches = self.repo.read().await.branches(Option::Some(BranchType::Local))
            .map_err(|e| From::from(e))?;

        let branch_name_list = branches.map(|branch| {
            let branch_name = branch.map_err(|e| Error::from(e))?
                .0.name()
                .map_err(|e| Error::from(e))?.unwrap().to_string();

            Ok(branch_name)
        }).collect::<Result<Vec<Branch>, Error>>();

        branch_name_list
    }

    /// Creates a branch on the commit.
    async fn create_branch(
        &self,
        branch_name: &Branch,
        commit_hash: CommitHash,
    ) -> Result<(), Error>{
        let oid = Oid::from_bytes(&commit_hash.hash).map_err(|e| Error::from(e))?;
        let commit = self.repo.read().await.find_commit(oid)
            .map_err(|e| Error::from(e))?;

        //if force true and branch already exists, it replaces with new one
        let _branch = self.repo.read().await.branch(
            branch_name.as_str(), 
            &commit,
            false
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    /// Gets the commit that the branch points to.
    async fn locate_branch(&self, branch: &Branch) -> Result<CommitHash, Error>{
        let branch = Repository::find_branch(
            &self.repo, 
            branch, 
            BranchType::Local
        ).map_err(|e| Error::from(e))?;
        let oid = branch.get().target().unwrap(); //TODO: ok_or
        let commit_hash = CommitHash{ hash: oid}; //TODO: convert Oid -> CommitHash

        Ok(commit_hash)
    }

    //TODO:
    /// Gets the list of branches from the commit.
    async fn get_branches(&self, commit_hash: &CommitHash) -> Result<Vec<Branch>, Error>{
        let branches = Repository::branches(
            &self.repo ,
            Option::Some(BranchType::Local)
        ).map_err(|e| Error::from(e))?;

        let aa = branches.filter(|&b| 
            b.unwrap().0.get().target().unwrap() == git2::Oid::from_bytes(&commit_hash.hash).unwrap()
        ).collect::<Result<Vec<(git2::Branch, BranchType)>, git2::Error>>();//.unwrap().unwrap().0;

        let k = aa.unwrap();

        let branch_name_list = k.iter().map(|&branch| {
            let b = branch.0.name()
                .map_err(|e| Error::from(e))?.unwrap().to_string();
                Ok(b)
        }).collect::<Result<Vec<Branch>, Error>>();

        branch_name_list
    }

    /// Moves the branch.
    async fn move_branch(&mut self, branch: &Branch, commit_hash: &CommitHash)
        -> Result<(), Error>{
            let branch = Repository::find_branch(
                &self.repo, 
                branch, 
                BranchType::Local
            ).map_err(|e| Error::from(e))?;
            let oid = Oid::from_bytes(&commit_hash.hash)
                .map_err(|e| Error::from(e))?;
            let reflog_msg = ""; //TODO: reflog_msg
            let _ = branch.get().set_target(oid, reflog_msg)
                .map_err(|e| Error::from(e));

            Ok(())
        }

    /// Deletes the branch.
    async fn delete_branch(&mut self, branch: &Branch) -> Result<(), Error>{
        let branch = Repository::find_branch(
            &self.repo, 
            branch, 
            BranchType::Local
        ).map_err(|e| Error::from(e))?;

        let _delete = branch.delete().map_err(|e| Error::from(e));

        Ok(())
    }

    // -------------------
    // Tag-related methods
    // -------------------

    /// Returns the list of tags.
    async fn list_tags(&self) -> Result<Vec<Tag>, Error>{
        //pattern defines what tags you want to get
        let tag_array=  Repository::tag_names(&self.repo, None)
            .map_err(|e| Error::from(e))?;

        let tag_list = tag_array.iter().map(|tag| 
            tag.unwrap().to_string() //TODO: ok_or
        ).collect::<Vec<Tag>>();

        Ok(tag_list)
    }

    /// Creates a tag on the given commit.
    async fn create_tag(&mut self, tag: &Tag, commit_hash: &CommitHash) -> Result<(), Error>{
        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;
        let commit = Repository::find_commit(&self.repo, oid)
            .map_err(|e| Error::from(e))?;

        //if force true and tag already exists, it replaces with new one
        let object = Repository::find_object(
            &self.repo, 
            oid, 
            Some(ObjectType::Commit)
        ).map_err(|e| Error::from(e))?;
        let tagger = self.repo.signature()
            .map_err(|e| Error::from(e))?;
        let tag_message = ""; //TODO: tag_message

        let _tag = Repository::tag(
            &self.repo, 
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
    async fn locate_tag(&self, tag: &Tag) -> Result<CommitHash, Error>{
        let references = Repository::references(&self.repo)
            .map_err(|e| Error::from(e))?;
        
        let refs = references.filter(|&reference| {
            reference.unwrap().is_tag()
        }).collect::<Vec<Result<git2::Reference, git2::Error>>>();

        let tags = refs.iter().map(|&x| 
            x.unwrap().peel_to_tag().unwrap()
        ).collect::<Vec<git2::Tag>>();

        let tags_filter = tags.iter().filter(|&tag_target| {
            let tag_name = tag_target.name().unwrap();
            tag_name == tag //TODO: &String cmp
        }).collect::<Vec<git2::Tag>>(); //TODO: type

        let oid = tags_filter[0].target().unwrap().id();
        let commit_hash = CommitHash{hash: oid}; //TODO: oid->CommitHash
        Ok(commit_hash)

        /*
        let oid = references.map(|res_reference| {
            let reference = res_reference.unwrap();
            if reference.is_tag(){
                let tag_target = reference.peel_to_tag().unwrap();
                let tag_name = tag_target.name().unwrap();
                if tag_name == tag{
                    tag_target.target().unwrap().id()
                }
            }
        }).collect::<git2::Oid>();*/
    }

    //TODO: unwrap()
    /// Gets the tags on the given commit.
    async fn get_tag(&self, commit_hash: &CommitHash) -> Result<Vec<Tag>, Error>{
        //tags from one commit
        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;

        let references = Repository::references(&self.repo)
            .map_err(|e| Error::from(e))?;
        
        let refs = references.filter(|&reference| {
            reference.unwrap().is_tag()
        }).collect::<Vec<Result<Reference, git2::Error>>>();

        let tags = refs.iter().map(|&x| 
            x.unwrap().peel_to_tag().unwrap()
        ).collect::<Vec<git2::Tag>>();

        let tags_filter = tags.iter().filter(|&&target| 
          target.target().unwrap().id() == oid
        ).collect::<Vec<git2::Tag>>(); //TODO: type
        //Repository::find_tag()

        let res = tags_filter.iter().map(|target| target.name().unwrap().to_string())
        .collect::<Vec<Tag>>();
        Ok(tags_filter)
    }

    /// Removes the tag.
    async fn remove_tag(&mut self, tag: &Tag) -> Result<(), Error>{
        self.repo.read().await.tag_delete(tag.as_str()).map_err(|e| Error::from(e))
    }
    // ----------------------
    // Commit-related methods
    // ----------------------

    /// Create a commit from the currently checked out branch.
    async fn create_commit(
        &mut self,
        commit_message: &str,
        diff: Option<&str>,
    ) -> Result<CommitHash, Error>{
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
        let parent_commit_hash = self.locate_branch(branch).await?; //TODO: find branch
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
        self.repo.reset(commit_new.as_object(), git2::ResetType::Soft, None);

    }


    /// Creates a semantic commit from the currently checked out branch.
    async fn create_semantic_commit(&mut self, commit: SemanticCommit)
        -> Result<CommitHash, Error>{
         /*   pub title: String,
            pub body: String,
            /// (If this commit made any change) the new reserved state.
            pub reserved_state: Option<ReservedState>,*/
        
            //commit message만 달라짐
            unimplemented!()    
        }

    /// Reads the reserved state from the current working tree.
    async fn read_semantic_commit(&self, commit_hash: &CommitHash)
        -> Result<SemanticCommit, Error>{
            unimplemented!()
        }

    /// Removes orphaned commits. Same as `git gc --prune=now --aggressive`
    async fn run_garbage_collection(&mut self) -> Result<(), Error>{
        unimplemented!()
    }

    // ----------------------------
    // Working-tree-related methods
    // ----------------------------

    /// Checkouts and cleans the current working tree.
    /// This is same as `git checkout . && git clean -fd`.
    async fn checkout_clean(&mut self) -> Result<(), Error>{
        unimplemented!()
        //reset unstaged files and remove untracked files including directory
        //TODO: check repo.statues() and statusOption
    }

    /// Checkouts to the branch.
    async fn checkout(&mut self, branch: &Branch) -> Result<(), Error>{
        let obj = Repository::revparse_single(
            &self.repo,
            &("refs/heads/".to_owned() + branch)
        ).map_err(|e| Error::from(e))?;

        Repository::checkout_tree(
            &self.repo,
            &obj,
            None
        ).map_err(|e| Error::from(e));

        Repository::set_head(
            &self.repo, 
            &("refs/heads/".to_owned() + branch)
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    /// Checkouts to the commit and make `HEAD` in a detached mode.
    async fn checkout_detach(&mut self, commit_hash: &CommitHash) -> Result<(), Error>{
        let obj = Repository::revparse_single(
            &self.repo, 
            &("refs/heads/".to_owned() + commit_hash.hash) //TODO: hash -> str
        ).map_err(|e| Error::from(e))?; 

        Repository::checkout_tree(
            &self.repo, 
            &obj,
            None,
        ).map_err(|e| Error::from(e));

        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;

        Repository::set_head_detached(&self.repo, oid)
            .map_err(|e| Error::from(e));

        Ok(())
        //https://stackoverflow.com/questions/55141013/how-to-get-the-behaviour-of-git-checkout-in-rust-git2
    }

    // ---------------
    // Various queries
    // ---------------

    /// Returns the commit hash of the current HEAD.
    async fn get_head(&self) -> Result<CommitHash, Error>{
        let ref_head = Repository::head(&self.repo)
            .map_err(|e| Error::from(e))?;
        let oid = ref_head.target().unwrap(); //TODO: ok_or

        Ok(CommitHash{hash: oid}) //TODO: Oid -> CommitHash
    }

    /// Returns the commit hash of the initial commit.
    ///
    /// Fails if the repository is empty.
    async fn get_initial_commit(&self) -> Result<CommitHash, Error>{
        //check if the repsotiroy is empty
        //TODO: is this right?
        
        let _head = Repository::head(&self.repo)
            .map_err(|_| Error::InvalidRepository("Repository is empty".to_string()))?;

        //TODO: A revwalk allows traversal of the commit graph defined by including one or
        //      more leaves and excluding one or more roots.
        //      --> revwalk can make error if there exists one or more roots...
        //if not
        let mut revwalk = Repository::revwalk(&self.repo)?;

        revwalk.push_head()
            .map_err(|e| Error::from(e))?;
        revwalk.set_sorting(
            git2::Sort::TIME | git2::Sort::REVERSE
        );

        let oids: Vec<Oid> = revwalk.by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()
            .map_err(|e| Error::from(e))?; //TODO: is this right?

        Ok(CommitHash{hash: oids[0]}) //TODO: oid -> CommitHash

        //https://users.rust-lang.org/t/make-sure-git2-revwalk-is-linear/25560/3
    }

    /// Returns the diff of the given commit.
    async fn show_commit(&self, commit_hash: &CommitHash) -> Result<String, Error>{
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
    async fn list_ancestors(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>{
        let oid = Oid::from_bytes(&commit_hash.hash)
            .map_err(|e| Error::from(e))?;
        let mut revwalk = Repository::revwalk(&self.repo)?;

        revwalk.push(oid)
            .map_err(|e| Error::from(e))?;
        revwalk.set_sorting(git2::Sort::TIME);

        //compare max and ancestor's size 
        //slice it

        let oids: Vec<Oid> = revwalk.by_ref()
            .collect::<Result<Vec<Oid>, git2::Error>>()
            .map_err(|e| Error::from(e))?; //TODO: is this right?




        let commit = Repository::find_commit(
            &self.repo, 
            Oid::from_bytes(&commit_hash.hash).map_err(|e| Error::from(e))?
        ).map_err(|e| Error::from(e))?;
        let num_parents = commit.parents().len(); 

        if num_parents == 0 {
            Err(Error::InvalidRepository("There is no parent commit".to_string()))
        }else if num_parents > 1 {
            Err(Error::InvalidRepository("There exists a merge commit".to_string()))
        }else{
            /* 
            //TODO: recursive? if nth ancestor faces merge commit during iteration, does it meaning fail?
            let mut num = 0;
            while num == max || {
                commit.parents()
            }*/
            
        }
    }

    /// Lists the descendant commits of the given commit (The first element is the direct child).
    ///
    /// It fails if there are diverged commits (i.e., having multiple children commit)
    /// * `max`: the maximum number of entries to be returned.
    async fn list_descendants(
        &self,
        commit_hash: &CommitHash,
        max: Option<usize>,
    ) -> Result<Vec<CommitHash>, Error>{
        unimplemented!()
    }

    /// Returns the children commits of the given commit.
    async fn list_children(&self, commit_hash: &CommitHash) -> Result<Vec<CommitHash>, Error>{
        unimplemented!()
    }

    /// Returns the merge base of the two commits.
    async fn find_merge_base(
        &self,
        commit_hash1: &CommitHash,
        commit_hash2: &CommitHash,
    ) -> Result<CommitHash, Error>{
        let oid1 = Oid::from_bytes(&commit_hash1.hash).map_err(|e| Error::from(e))?;
        let oid2 = Oid::from_bytes(&commit_hash2.hash).map_err(|e| Error::from(e))?;

        let oid_merge = Repository::merge_base(&self.repo, oid1, oid2)
            .map_err(|e| Error::from(e))?;
        let commit_hash_merge: [u8; 20] = oid_merge.as_bytes().try_into().unwrap(); //TODO: type right?

        Ok(CommitHash{hash: commit_hash_merge})
    }

    // ----------------------------
    // Remote-related methods
    // ----------------------------

    /// Adds a remote repository.
    async fn add_remote(&mut self, remote_name: &str, remote_url: &str) -> Result<(), Error>{
        let _remote = Repository::remote(
            &self.repo, 
            remote_name, 
            remote_url
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    /// Removes a remote repository.
    async fn remove_remote(&mut self, remote_name: &str) -> Result<(), Error>{
        let _remote_delete = Repository::remote_delete(
            &self.repo, 
            remote_name
        ).map_err(|e| Error::from(e))?;

        Ok(())
    }

    /// Fetches the remote repository. Same as `git fetch --all -j <LARGE NUMBER>`.
    async fn fetch_all(&mut self) -> Result<(), Error>{
        //1. get all of remote repository name and its branches which are used below
        //git fetch origin/main == repo.find_remote("origin")?.fetch(&["main"], None, None)
        //TODO: &["*"] works? or should find (remote, branch) ...
        unimplemented!()
    }

    //TODO: unwrap()
    /// Lists all the remote repositories.
    ///
    /// Returns `(remote_name, remote_url)`.
    async fn list_remotes(&self) -> Result<Vec<(String, String)>, Error>{
        let remote_array = Repository::remotes(&self.repo)
            .map_err(|e| Error::from(e))?;

        let remote_name_list = remote_array.iter().map(|remote| 
            remote.unwrap().to_string()
        ).collect::<Vec<String>>();

        let res = remote_name_list.iter().map(|&name|{
            let remote = Repository::find_remote(
                &self.repo, 
                name.as_str()
            ).map_err(|e| Error::from(e))?;

            let url = remote.url()
                .ok_or_else(|| Error::Unknown("unable to get valid url".to_string()))?;
                //.map_err(|e| Error::Unknown(e))?;

            Ok((name, url.to_string()))
        }).collect::<Result<Vec<(String, String)>, Error>>();

        res
    }

    /// Lists all the remote tracking branches.
    ///
    /// Returns `(remote_name, remote_url, commit_hash)`
    async fn list_remote_tracking_branches(
        &self,
    ) -> Result<Vec<(String, String, CommitHash)>, Error>{
        unimplemented!()
        //TODO: remote_name - branch ??
        //1. get (remote_name, remote_url) from list_remotes
        //2. can get commit object from rev_single but don't know what remote contains what branches
        //branches by type remote can get remote branches but don't know each branches' remote name
    }
}