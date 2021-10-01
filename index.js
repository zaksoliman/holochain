const { spawn } = require('child_process')
const { createWriteStream } = require('fs')
const { promises: { mkdir, rmdir } } = require('fs')

const { AdminWebsocket } = require('@holochain/conductor-api')

const { init: initShim } = require('./shim')

const wait = ms => new Promise(resolve => setTimeout(resolve, ms))

const runCommand = (command, ...args) => {
    const process = spawn(command, args)
    const dead = new Promise(resolve => process.once('exit', resolve))
    const outfile = createWriteStream(`./log/${command}.txt`)
    process.stdout.pipe(outfile)
    process.stderr.pipe(outfile)
    const kill = async () => {
        process.kill()
        await dead
    }
    return kill
}

const main = async () => {
    let killLair = null
    let shim = null
    let killHolochain = null
    await rmdir('./tmp', { recursive: true })
    await mkdir('tmp')
    await mkdir('log', { recursive: true })
    let disconnectedAgent = null
    try {
        // Spawn lair
        killLair = runCommand('lair-keystore', '--lair-dir', './tmp/keystore')
        await wait(1_000)
        // Setup lair-shim
        const shimCb = (agent/*: Buffer*/, payload/*: any*/) => {
            // Either return null or return Promise<signature>
            if (disconnectedAgent && disconnectedAgent.includes(agent)) {
                console.log('purposeful signing error')
                return Promise.reject('purposeful signing error')
            }
            return null
        }
        await mkdir('./tmp/shim')
        shim = initShim('./tmp/keystore/socket', './tmp/shim/socket', shimCb)
        await wait(1_000)
        // Spawn holochain
        killHolochain = runCommand('holochain', '--config-path', 'holochain-config.yml')
        await wait(5_000)
        console.log("Install dummy DNA")
        const adminWs = await AdminWebsocket.connect("ws://127.0.0.1:4444/")
        const agent1 = await adminWs.generateAgentPubKey()
        const agent2 = await adminWs.generateAgentPubKey()
        console.log('agent1', agent1)
        console.log('agent2', agent2)
        await adminWs.installAppBundle({
            installed_app_id: "happ-1",
            agent_key: agent1,
            membrane_proofs: {
                'test': Buffer.from('rGpvaW5pbmcgY29kZQ==', 'base64')
            },
            path: './test.happ'
        })
        console.log("Activate app")
        await adminWs.activateApp({
            installed_app_id: "happ-1"
        })
        console.log("Turn off signing for agent #1")
        disconnectedAgent = Buffer.from(agent1)

        console.log("Deactivate App for agent #1")
        await adminWs.deactivateApp({
            installed_app_id: "happ-1"
        })
        // ^ Fails

        console.log('Apps:')
        console.log(await adminWs.listApps({}))
    } catch(e) {
        console.log('ERROR ****************')
        console.log(e)
    } finally {
        await wait(3_000)
        console.log('Cleaning up')
        if (shim) {
            await (await shim).stop()
        }
        if (killHolochain) {
            await killHolochain()
        }
        if (killLair) {
            await killLair()
        }
    }
}

main()
