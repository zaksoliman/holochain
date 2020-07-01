# Notes about HDK compatibility/changes

## Beware of Possible breaking issues (required for HDK 2.5)
 - [ ] Encoding: Binary/MessagePack/Serialized Bytes instead of JSON
 - [ ] Direct API Access: 
 - [x] **New header format:** The header will include a new sequence field for the header sequence number in the chain to simplify validation logic and rollback detection in the DHT.
 - [x] **Provenance should instead use headers:** Because of new self-validating header structure, we may need to repoint references to signatures or provenance with some backward compatibility code.
 - [ ] **HDK support for validation receipts & warrants:** Network message protocols will likely change. There are certainly some new ones (like for validation receipts and warrants and such), but also the expected data structures of previous ones may chnage a bit with the new workflows.

## Non-Breaking changes for HDK 2.5
 - [ ] **Validation Package** should be able to be specified in entry def as: Entry, Full(chain), Partial(chain), or Custom. And ONLY CUSTOM packages should make a call into WASM to be generated. Others should be generated from the Rust side when handing a Package request. This package type should exposed OUTSIDE of WASM along with Zome functions and defined callbacks.
 - [ ] **Don't request validation packages we already have:** We should normally make a network call for getting Validation Package, however, we can first locally inspect our exposed entry-dev package types to see what type of package we need. If we already have the entry&header and that is the validation type, we don't need to request a package.


## Desired changes to HDK 3.0

 - [ ] **Native counter-signing:** recognition of counter-signing for explicit calls to EACH agent who is party to a multi-party transaction is the only way to get ALL the validation packages required.
 - [ ] **Simplified Gets:** simplify get calls to: 1. A get enumerated function with different sigs for the different gets. 2. A get_links enumerated function with different sigs for different gets.
 - [ ] **Simplified Get_Links:** just two commands as `get` above. 1. get_link default behavior (most recent n(=50?) links sorted by date)
 - [ ] **Simplify call command** to context + fn instead of 5 parameters. `Context=dna,zome,capToken Call=fn,params`
 - [ ] **Send/Receive becomes call_remote** What was formerly send/receive is replaced by the ability to do send a remote zome call to another node. Standard capability tokens enforce the permissions. Only calls explicity exposed to "ANYONE" can be made without a capability claim.
 - [ ] **Separate link defs from entry defs**. And have strong typing of from/to be optional not required.
 - [ ] **[Entry defs](https://hackmd.io/BADM1Nr1Tq6rmO8CnadTzQ?both)** Now require more than just a struct for their content/fields:
     - Encoded in Entry Def level
         - "CRDT Type" for any special managed behavior from Holochain (such as "single author entry")
         - Validation bundle # (0 or NULL for no bundle needs to be constructed/gossiped)
         - Private/Public
         - Other possible stuff: UI Label, display widget identifier, edit widget identifier, sample values (used for tests).
         - Constructor for deserializing from SB which contains... 
     - Encoded into Entry itself
         - enum with which type/zome
         - Content fields / struct
 - [ ] **Link Defs** 
     - Links will no longer be defined as part of an entry. Rather there will be a function defined for validating their structure... the function name will replace what was formerly "type"
     - There will be a monolithic validate_links callback function which will switch on the "type" to perform validation. This may get constructed for them out of a macro-ized HDK approach.
 - [x] **Rename: ChainHeader** `link`, `link_same_type`, `link_update_delete` fields to clarify purpose and not confuse them with actual links. (e.g. `prev_header`, `prev_same_type`, `replaced_entry`)
 - [x] **Rename fields in links also**. LinkData contents force you to write `link.link.base()` code that is not very understandable. Should be base_address, target_address, etc.
 - [ ] **Access to Some Basic System Fns** to make life easier for devs: 
     - current system time (UTC?), 
     - random number generator, 
     - and UUID generator?
 

------
### Callback Infrastructure / Hooks

We need a generalized way of sparsely defining callback functions that can be called from various parts of the data propagation process.

This might be able to be specified using similar format of hc_public in our fn def macros.

We need callbacks available for so many things! Zomes should be able to specify calbacks for some conductor or network events:
 - [x] **Genesis** / Install App (optional)
 - [x] **Init,** / First Run (optional)
 - Startup (skip for now?), 
 - Join Network (skip for now?),
 - Join App Space (skip for now?), 
 - Leave App Space (skip for now?), 
 - Leave Network (skip for now?),
 - Shutdown (skip for now?),
 - Open / Close: Migrate Agency
 - Migrate Data  (skip for now?),
 - Uninstall / Remove App (skip for now?)

Then there are data events needed per Entry & Link type x Action (Create, [Update,] Delete):
 - [x] **Validate** -- ***ONLY ONE REQUIRED*** (e.g. UserName_Validate_Create, UserName_Validate Update, UserName_Validate_Delete) Note: All validation rules on an action must pass, if any fail, it is considered invalid.
     - Validation Failure --> Warrant?
     - Validation Abandon --> Notification?
     - Validation Success --> Receipt?
 - [x] **Produce Validation Package** (optional default to entry_with_header) ONLY needs to be defined for CUSTOM validation package types, others (Entry, Full, Partial) should be able to be generated via Rust without calling into WASM.
 - Pre-Commit (skip for now?) - lets you add something which would get committed in the same TX workspace bundle as the changes triggering this hook. 
 - [x] **Post-Commit(vec<element_hashes_commited>)** - lets you trigger something after workspace commits validate (such as sending data by direct message to an indexing node).
 - Publish to DHT  (skip for now?) (maybe we should remove this and use StoreEntry in DHT actions below?)

Also on System Entry Types?: %dna, **%key,** %header, %cap_grant, etc...

> The callbacks on DHT ops seems like overkill to me since no appreciable time passes between the corresponding commit action and the dht op generation, and I can't think of a use case for that specificity..
> [name=Michael Dougherty]

> Sample USE CASE for DHT Ops.
>  - Upon HoloFuel FinalizeTx_StoreHeader_Receive have the nodes who receive the Tx headers also run the validation rules on the transaction so that a transaction cannot be pre-imaged to control the neighborhood of validators. It creates three neighborhoods (1 for tx 2 for headers) that you'd have to try to control.
> 
> But you're right. In general, I don't think the demand is high for this yet. 
> 
> Although I hope the use case for the DHT_Conflict_Detection callback is clear.
> [name=Arthur Brock]

We may also want callback events on DHT Transforms distinct from entries. DHT_Op x Send/Receive?
 - StoreHeader (e.g. UserName_StoreHeader_Send, UserName_StoreHeader_Receive)
 - StoreEntry
 - RegisterAgentActivity
 - RegisterUpdatedTo
 - RegisterDeletedBy
 - RegisterAddLink
 - RegisterRemoveLink 
 - [ ] DHT Conflict Detection (Partition Resolution)

Do we want to allow hooks on entering ZomeFn processing?
 - call() --

If a callback procmacro explicitly references a zome, you could attach hooks on another zome's activities, without messing with the source code of that zome. Such as: ZomeName:EntryType:ActionType:Eventtype (e.g. Anchors::Anchor:Create:Validate, or Roles::AdminMember:Remove:Pre-Commit )

### Native support for Multi-Party signing

The manual process of managing multi-party signing (such as on a HoloFuel transaction, Holo-REA accounting, or a legal contract) is challenging to do manually. It seems to generally require a series of pre-authorizing transactions (one on both side) so that nodes can then construct, negotiate, and sign to finalize the transaction without the delays of human interaction. We assume that HoloFuel will work, but so far -- no joy.

I think there's a generalized pattern that we could code for the subconscious to execute and complete these transactions.

We could use the Proc Macro pattern to identify a rust struct that contains the addresses of all the parties involved.

```rust
pub type RequiredSigner = Address;

#[..., MultipartyEntry]
pub struct Transaction {
    sender: RequiredSigner,
    receiver: RequiredSigner
}
```

We expect different implementations and behaviour will pop up from hApp devs and we can integrate what works best. This is a proposal of how the process could look like:

1. Alice calls hdk::begin_multiparty_entry({ sender: Alice, receipient: Bob }). This creates:
* Entry as a private entry on alice's source chain
* Header for that entry, proving that alice signed the entry.
* TBD: do we need to create a CapabilitiesGrant?
2. Alice node sends a notification to Bob's node, so that when it's online it prompts to finish the process.
3. Bob calls hdk::complete_multipary_entry(entry), creating the same private entries, and trying to see if Alice is online to finish automatically the entry.
4. The entry is committed with the signatures of all participants, having the system validation subconscious rules check that all parties' headers are present before beggining the app's validation. 