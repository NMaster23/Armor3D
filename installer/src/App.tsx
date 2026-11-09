import { useEffect, useState } from "react";
import "./App.css";
import { Card, ProgressBar, ScrollShadow, Typography } from "@heroui/react";
import { invoke } from "@tauri-apps/api/core";
import { Check } from "@gravity-ui/icons";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

const system_checks_text = [
  "Auditing your PATH variable and quietly judging your life choices",
  "Asking Python 2.7 to please leave the premises",
  "Checking if you actually have 16 GB of RAM or if Chrome is just lying to you",
  "Scanning for abandoned node_modules folders to cannibalize for scrap metal",
  "Consulting a 2011 forum thread marked 'Fixed it, nvm' with zero follow-up",
  "Verifying that your CPU fan is spinning fast enough to double as a hovercraft",
  "Negotiating an uneasy truce with your antivirus"
];

const downloading_texts = [
  "Unstrangling stray TCP packets that arrived out of order",
  "Downloading 400 MB of dependencies to left-pad a single string",
  "Siphoning extra bandwidth from an open Wi-Fi network down the street",
  "Decompressing 47 recursive tarballs just to reach a single .env.example file",
  "Converting bits into bytes, then converting bytes back into excuses",
  "Re-encrypting already-decrypted decryptors",
  "Polling the server every 2 milliseconds to make sure it doesn’t feel lonely",
];

const compiling_texts = [
  "Reticulating splines (mandatory by municipal code)",
  "Compiling your code into a single, incomprehensible blob of regret",
  "Transpiling TypeScript into JavaScript, then back into TypeScript for fun",
  "Optimizing your code until it runs faster than the speed of light (or at least until it compiles)",
  "Running a quantum algorithm to predict the future of your code",
  "Summoning a team of code gnomes to manually review every line",
  "Converting your code into a series of interpretive dance moves for better performance",
  "Teaching C++ how to feel joy again",
  "Resolving circular dependencies by turning them into an octagon",
  "Staring directly at the progress bar to make it move slower",
];

const finishing_texts = [
  "Convincing Windows Defender to install malware",
  "Polishing the toolbar with a microfiber cloth",
  "Bribing your GPU with extra VRAM",
  "Asking the high council for .3dm",
  "Ready for launch!"
];

function App() {
  const [state, setState] = useState(0);
  const [installingState, setInstallingState] = useState(0);
  const [isLoaded, setIsLoaded] = useState(false);
  const fullText = " The Next Generation of Computer Aided Design";
  const [displayText, setDisplayText] = useState("");
  const [isButton, setIsButton] = useState(false);
  const [loadText, setLoadText] = useState("Preparing...");
  const [progress, setProgress] = useState(0.0);
  const [installPath, setInstallPath] = useState("");
  const [freeGB, setFreeGB] = useState(0);
  const [createShortcut, setCreateShortcut] = useState(true);
  useEffect(() => {
    const timer = setTimeout(() => setIsLoaded(true), 50);
    invoke<{ default_dir: string }>("get_default_paths").then(async (paths) => {
      setInstallPath(paths.default_dir);
      const freeBytes = await invoke<number>("check_disk_space", { targetPath: paths.default_dir });
      setFreeGB(Math.round((freeBytes / (1024 * 1024 * 1024) * 10) / 10));
    }).catch((err) => console.error(err));
    return () => clearTimeout(timer);
  }, []);
  useEffect(() => {
    if (progress <= 33) {
      setInstallingState(0);
    } else if (progress > 33 && progress <= 66) {
      setInstallingState(1);
    } else if (progress > 66 && progress <= 90) {
      setInstallingState(2);
    } else if (progress > 90) {
      setInstallingState(3);
    } else {
      invoke("save_config");
    }
  }, [progress]);
  useEffect(() => {
    if (installingState === 0) {
      const timer = setInterval(() => {
        setLoadText(system_checks_text[Math.floor(Math.random() * system_checks_text.length)]);
      }, 1000);
      return () => clearInterval(timer);
    } else if (installingState === 1) {
      const timer = setInterval(() => {
        setLoadText(downloading_texts[Math.floor(Math.random() * downloading_texts.length)]);
      }, 1000);
      return () => clearInterval(timer);
    } else if (installingState === 2) {
      const timer = setInterval(() => {
        setLoadText(compiling_texts[Math.floor(Math.random() * compiling_texts.length)]);
      }, 1000);
      return () => clearInterval(timer);
    } else if (installingState === 3) {
      const timer = setInterval(() => {
        setLoadText(finishing_texts[Math.floor(Math.random() * finishing_texts.length)]);
      }, 1000);
      return () => clearInterval(timer);
    }
  }, [installingState]);

  useEffect(() => {
    let index = 0;
    const interval = setInterval(() => {
      index++;
      setDisplayText(fullText.slice(0, index));
      if (index >= fullText.length) {
        clearInterval(interval);
        setTimeout(() => {
          setIsButton(true);
        }, 300);
      }
    }, 40);
    return () => clearInterval(interval);
  }, []);
  useEffect(() => {
    if (state === 1) {
      const listenProgress = listen<{ progress: number; msg: string }>(
        "install-progress",
        (event) => {
          setProgress(event.payload.progress);
          setLoadText(event.payload.msg)
        }
      );
      invoke<{ default_dir: string }>("get_default_paths")
        .then((paths) => {
          return invoke("install_app", { targetDir: paths.default_dir, createShortcut: createShortcut });
        })
        .catch((err) => {
          setLoadText(`Installation failed: ${err}`);
        });
      return () => {
        listenProgress.then((listen) => listen());
      }
    }
  }, [state])
  return (
    <div
      className={`w-screen h-screen flex items-center justify-center bg-black/40 transition-opacity ease-in-out delay-150 duration-[1500ms] ${
        isLoaded ? "opacity-100" : "opacity-0"
      }`}
    >
      {state === 0 ? (
        <div className="flex flex-col items-center justify-center">
          <h1
            className="text-center px-16 max-w-[90vw] bg-gradient-to-r from-white via-blue-200 to-zinc-400 bg-clip-text text-transparent"
            style={{ marginTop: "8px", fontSize: "7rem", lineHeight: 1.15 }}
          >
            {displayText}
            {!isButton && (
              <span className="inline-block w-[10px] h-[0.85em] bg-blue-400 ml-3 align-middle rounded-sm animate-pulse" />
            )}
          </h1>
          {installPath && (
            <div className="flex items-center gap-4 text-xs font-mono text-zinc-400 bg-zinc-900/80 px-4 py-2 rounded-xl mt-6">
              <span className="truncate max-w-[320px] text-zinc-300">{installPath}</span>
              <span className="text-zinc-400 font-semibold">{freeGB} GB Free</span>
            </div>
          )}
          <label className="flex items-center gap-2 mt-4 text-xz text-zinc-400 cursor-pointer select-none">
            <input 
              type="checkbox"
              checked={createShortcut}
              onChange={(e) => setCreateShortcut(e.target.checked)}
              className="rounded w-3.5 h-3.5 cursor-pointer"
            />
            <span>Create a Desktop Shortcut</span>
          </label>
          <div
            onClick={() => {
              if (isButton) {
                setState(1);
              }
            }}
            className={`transition-all duration-700 ease-[cubic-bezier(0.16,1,0.3,1)] flex items-center justify-center select-none ${
              isButton
                ? "w-[220px] h-[56px] bg-blue-500 hover:bg-blue-400 hover:scale-105 active:scale-95 rounded-full cursor-pointer shadow-xl shadow-blue-500/30 mt-12 opacity-100 translate-y-0"
                : "w-[10px] h-[50px] bg-blue-400 rounded-sm opacity-0 -translate-y-8 pointer-events-none"
            }`}
          >
            <span
              className={`text-white text-lg font-medium tracking-wide transition-opacity duration-300 delay-300 ${
                isButton ? "opacity-100" : "opacity-0"
              }`}
            >
              Continue &rarr;
            </span>
          </div>
        </div>
      ) : (
        <div className="dark w-[860px] h-[520px] bg-[#16161a] border border-[#2a2a32] rounded-2xl shadow-2xl p-8 text-white">
          <Typography type="h1">{loadText}</Typography>
          <div className="w-full mt-4">
            <ProgressBar value={progress} className="w-full">
              <div className="flex justify-between items-center mb-2 text-sm text-zinc-400">
                <span>{loadText}</span>
                <ProgressBar.Output className="font-mono text-cyan-400 font-medium" />
              </div>
              <ProgressBar.Track className="h-3 w-full bg-zinc-800 rounded-full overflow-hidden">
                <ProgressBar.Fill className="h-full bg-gradient-to-r from-blue-500 to-cyan-400 rounded-full transition-all duration-150" />
              </ProgressBar.Track>
            </ProgressBar>
          </div>
          <Card className="w-full p-0">
            <ScrollShadow className="max-h-[240px] p-4" orientation="vertical">
              {(installingState === 0
                ? system_checks_text
                : installingState === 1
                ? downloading_texts
                : installingState === 2
                ? compiling_texts
                : finishing_texts
              ).map((text, idx) => (
                <div key={idx} className="flex items-center space-x-3 text-sm">
                  {text === loadText ? (
                    <span className="text-cyan-400 animate-pulse font-bold">&gt;</span>
                  ): (
                    <Check className="w-4 h-4 text-green-400"/>
                  )}
                  <span className="truncate">{text}</span>
                </div>
              ))}
            </ScrollShadow>
          </Card>
          {progress >= 100 && (
            <button
              onClick={async () => {
                const paths = await invoke<{ default_dir: string }>("get_default_paths");
                await invoke("launch_installed_app", {
                  exePath: `${paths.default_dir}\\Armor3D.exe`,
              });
              await getCurrentWebviewWindow().close();
              }}
              className="mt-6 w-full py-3 bg-blue-500 hover:bg-blue-600 text-white font-semibold rounded-xl shadow-lg shadow-cyan-500/20 cursor-pointer active:scale-98 transition-all">
                Launch Armor3D &rarr;
              </button>
          )}
        </div>
      )}
    </div>
  );
}

export default App;
