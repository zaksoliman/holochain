# Signing failure reproduction

Steps
1. `nix-shell`
2. `yarn install`
3. `node index.js`


Expected outcome:
Test completes successfully without printing `ERROR ****************`
Even though a signing request is failing for agent 2, I expect agent 3 to work be able to be activated.

Actual outcome:
Test errors with the following because agent 3 cannot be activated
```
init wormhole
New conductor connections
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
Install dummy DNA
Forwarding message to Lair
Forwarding message to Lair
Forwarding message to Lair
agent1 <Buffer 84 20 24 4b b4 e8 25 3d e3 a1 ed ad b2 39 50 dc cf 8a a0 f6 11 fb 82 e2 42 fa 49 54 2f ad 6d 09 f3 db b1 f6 8e 6a 4c>
agent2 <Buffer 84 20 24 e9 a9 79 f9 30 be 4c 92 74 fc 9d 0c d1 c9 7c a7 23 a4 77 c1 b7 e4 94 23 d1 fb 1c 73 23 73 90 f2 4b 28 20 86>
agent3 <Buffer 84 20 24 53 3d ad c2 33 f9 6b 1a 2d 0f 29 7a de 2d c0 89 e3 23 dd f8 ca 28 fa fc bf b9 13 78 18 50 e0 00 2a 13 52 32>
Install agent #1
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Activate agent #1
Intercepted sign by public key
Forwarding message to Lair
Install agent #2
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Activate agent #2
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Turn off signing for agent #2
Deactivate App for agent #2
Intercepted sign by public key
purposeful signing error
Wormhole failure: 'purposeful signing error'
Deactivate app failed with
{
  type: 'error',
  data: {
    type: 'internal_error',
    data: 'Conductor returned an error while using a ConductorApi: InternalCellError(HolochainP2pError(OtherKitsuneP2pError(KitsuneError(KitsuneError(Other(Other(GhostError(Disconnected))))))))'
  }
}
Apps:
[
  {
    installed_app_id: 'happ-1',
    cell_data: [ [Object] ],
    status: { running: null }
  },
  {
    installed_app_id: 'happ-2',
    cell_data: [ [Object] ],
    status: { disabled: [Object] }
  }
]
Install dummy DNA again
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Intercepted sign by public key
Forwarding message to Lair
Activate app #3
Intercepted sign by public key
purposeful signing error
Wormhole failure: 'purposeful signing error'
ERROR ****************
{
  type: 'error',
  data: {
    type: 'internal_error',
    data: 'Conductor returned an error while using a ConductorApi: InternalCellError(HolochainP2pError(OtherKitsuneP2pError(KitsuneError(KitsuneError(Other(Other(GhostError(Disconnected))))))))'
  }
}
Cleaning up
Stopping wormhole
```
