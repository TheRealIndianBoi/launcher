import { message } from "@tauri-apps/api/dialog";
import { platform } from "@tauri-apps/api/os";
import { appDir, join } from "@tauri-apps/api/path";
import { Command } from "@tauri-apps/api/shell";

export async function buildGame(callback) {
    const userPlatform = await platform();
    const jakLaunchPath = await join(await appDir(), '/jak-project/out/build/Release/bin/');

    let compilerScript = null;

    if (userPlatform === 'win32') {
        compilerScript = 'compile-windows';
    } else if (userPlatform === 'linux') {
        compilerScript = 'compile-linux';
    } else if (userPlatform === 'darwin') {
        compilerScript = 'compile-mac';
    }

    // if (compilerScript) {
    //     // so its not console logging the '100%' when i run it in the series, but when i run it on its own its fine.
    //     // so im going to assume its working properly and its a problem with the way the compiler is outputting the %%%
    //     // for now i have a timeout that will kill the compiler process after 30 seconds because the compiler should be done by then (twice the length it takes my pc at least)
    //     let build = execFile(compilerScript, ['-v', '-auto-user'], { timeout: 30000 });
    //     build.stdout.on('data', data => {
    //         console.log(data.toString().trim());
    //         app.emit('console', data);
    //         if (data.includes('[100%]')) {
    //             updateStatus('Compiled game successfully!');
    //             callback(null, 'Compiled game successfully!');
    //             return;
    //         }
    //     });

    //     build.on('close', () => {
    //         updateStatus('Compiled game successfully!');
    //         callback(null, 'Compiled game successfully!');
    //         return;
    //     });

    //     let stdinStream = new stream.Readable();
    //     stdinStream.push('(mi)');
    //     stdinStream.push(null);
    //     stdinStream.pipe(build.stdin);
    // }

    if (compilerScript) {
        const compile = new Command(compilerScript, null, { cwd: jakLaunchPath });
        compile.on('close', data => {
            console.log(`Compiler finished with code ${data.code} and signal ${data.signal}`);
            message('Game Ready to play!');
            return ('Compiler finished');
        });
        compile.on('error', error => {
            console.error(`Compiler error: "${error}"`);
        });
        compile.stdout.on('data', line => console.log(`Compiler stdout: "${line}"`));

        const child = await compile.spawn();
    }
}

export async function launchGame() {
    const userPlatform = await platform();
    const jaklaunchPath = await join(await appDir(), '/jak-project/scripts/batch/');
    let launchScript = null;

    if (userPlatform === 'win32') {
        launchScript = 'launch-windows';
    } else if (userPlatform === 'linux') {
        launchScript = 'launch-linux';
    } else if (userPlatform === 'darwin') {
        launchScript = 'launch-mac';
    }

    console.log(launchScript);

    if (launchScript) {
        const launch = new Command(launchScript, null, { cwd: jaklaunchPath });
        launch.on('close', data => {
            console.log(`Launch finished with code ${data.code} and signal ${data.signal}`);
            return ('Launch finished');
        });
        launch.on('error', error => {
            console.error(`Launch error: "${error}"`);
        });
        launch.stdout.on('data', line => console.log(`Launch stdout: "${line}"`));

        const child = await launch.spawn();
    }
}