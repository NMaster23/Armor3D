import { useEffect, useState } from "react";
import "./App.css";

function App() {
  const [state, setState] = useState(0);
  const [isLoaded, setIsLoaded] = useState(false);
  const fullText = " The Next Generation of Computer Aided Design";
  const [displayText, setDisplayText] = useState("");
  const [isButton, setIsButton] = useState(false);

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
        // Wait 600ms after typing finishes, then trigger morph into Continue button
        setTimeout(() => {
          setIsButton(true);
        }, 600);
      }
    }, 40);
    return () => clearInterval(interval);
  }, []);

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
        <div className="w-[860px] h-[520px] bg-[#16161a] border border-[#2a2a32] rounded-2xl shadow-2xl p-8 text-white">
          <h1 style={{ margin: 0, fontSize: "2rem", fontWeight: 600 }}>Setup Armor3D</h1>
        </div>
      )}
    </div>
  );
}

export default App;
