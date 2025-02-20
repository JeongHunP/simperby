# Simperby: the Git-based Blockchain

One of the most unique features of Simperby is its use of Git.

## Pre-requisites

You should have a basic understanding of

1. Blockchain
2. [Simperby Consensus and Governance](./protocol_overview.md)
3. Git

## Summary

1. **Every finalized data in a Simperby chain is stored in a Git repository.**
2. It includes transactions, agendas, agenda proofs, chats, and blocks, linearly committed in the `main` branch.
3. A Simperby node manages its own Git repository and provides a Git server of it to the node operator.
4. The node operator can fetch the blockchain data, walk through the history, and check the diffs using the Git protocol.
5. **Every to-be-finalized data is also managed in a Git repository.**
6. All the pending agendas (waiting for approval) will be presented as multiple branches grown from the `main` branch.
7. The node operator may create their own transaction as a commit and push to a particular branch which represents an agenda proposal.
8. **Simperby functions as a general distributed Git repository**, that may contain any useful data for the organization. This is trivially achieved because **we take transactions as Git commits.** (This can be understood as exploiting the 'blockchain state' as a Git repository)

## Lifecycle of a Simperby Chain

TODO

### Step 0: finalized block

Let's assume that there is the last finalized block with the height of `H`.
We will take that as a starting point of our recursive process. Of course, the base case of the finalized block would be the genesis block.

### Step 1: Agenda

TODO

## Specification

Here we present the specification of the Simperby Git repository.

### Commits

A commit is defined as follows

1. `initial`: an empty, initial commit as the very first commit of the repository.
2. `genesis`: a non-empty commit that contains the initial state and the genesis info.
3. `block`: an empty commit for the either proposed or finalized block
4. `tx`: a transaction of an arbitrary update on the state (except the reserved directory). Note that a `tx` commit is the only exception that the commit title does not start with its type, `tx`. It may be empty.
5. `tx-delegate`, `tx-undelegate`: a non-empty extra-agenda transaction that updates the delegation state which resides in the reserved directory of the repository.
6. `tx-report`: a non-empty commit that reports the misbehavior of a validator with cryptographic proof. This must include the state change caused by the slashing.
7. `chat`: an empty commit for the chat logs of the height.
8. `agenda-proof`: an empty commit for the proof of the governance approval of an agenda.

### Commit Format

### Branches

These are the names of the branches that are specially treated by the Simperby node. Branches other than `work` and `p` are managed by the node; it will be rejected if pushed.

1. `main`: always points to the last finalized block. It is strongly protected; users can't push to this branch.
2. `work`: the branch that users can freely push or force-push. CLI commands like `create` interact with this.
3. `p`: the block proposal for this node. The node operator may push or force-push to this branch. When pushed, the Git server will check the validity of the branch. The consensus engine will recognize this branch and propose to the consensus. It stands for 'block proposal'.
4. `a-<number>`: a valid agenda (but not yet approved) propagated from other nodes. If the governance has approved the agenda, it will point to the `agenda-proof` commit which lies on top of the agenda commit. The number is arbitrarily assigned.
5. `b-<number>`: a valid (but not yet finalized) block propagated from other nodes. The number is arbitrarily assigned.

### Tags

Tags can't be pushed by the users. They are always managed by the nodes.

1. `vote-<number>`: for agenda commits only; denotes that the user has voted for the agenda.
2. `veto-<number>`: for block commits only; denotes that the user has vetoed the block.

### Structure

```text
// The history grows from the bottom to the top.
// Each line represents a Git commit.

block H+1 (branch: main)
chat H+1
[extra-agenda transactions]
...
agenda proof H+1
agenda H+1
[ordinary transactions]
...
block H
```

If the node receives multiple agendas, it presents multiple branches that consist of `ordinary transactions` and a single `agenda` grown from `block`.

### Example

If an organization using Simperby keeps its repository public, it is natural to have a mirror of the block data repository on a publicly hosted service like Github.

We present an example of the block data [here](https://github.com/postech-dao/simperby-git-example)
