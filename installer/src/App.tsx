import { useEffect, useState } from "react";
import "./App.css";
import { Card, Label, ProgressBar, ScrollShadow, Typography } from "@heroui/react";
import { __unstable__loadDesignSystem } from "tailwindcss";

function App() {
  const [state, setState] = useState(0);
  const [isLoaded, setIsLoaded] = useState(false);
  const fullText = " The Next Generation of Computer Aided Design";
  const [displayText, setDisplayText] = useState("");
  const [isButton, setIsButton] = useState(false);
  const [loadText, setLoadText] = useState("Preparing...");
  const [progress, setProgress] = useState(0.0);
  useEffect(() => {
    const timer = setTimeout(() => setIsLoaded(true), 50);
    return () => clearTimeout(timer);
  }, []);

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
      const timer = setInterval(() => {
        setProgress((prev) => {
          if (prev >= 100) {
            clearInterval(timer);
            return 100;
          }
          return prev + 2;
        });
      }, 50);
      return () => clearInterval(timer);
    }
  }, [state]);
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
              <div className="space-y-4">
                {Array.from({length: 10}).map((__unstable__loadDesignSystem, idx) => (
                  <p key={`scroll-shadow-lorem-content-${idx}`}>
                    
                  </p>
                ))}
              </div>
            </ScrollShadow>
          </Card>
        </div>
      )}
    </div>
  );
}

export default App;
