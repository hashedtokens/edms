import React from 'react';
import { useMousePosition } from '../../hooks/useMousePosition';

export const BackgroundEffects = () => {
  const mousePosition = useMousePosition();

  return (
    <>
      <div 
        className="fixed top-0 left-0 w-full h-full opacity-20 pointer-events-none z-10"
        style={{
          background: `radial-gradient(600px circle at ${mousePosition.x}px ${mousePosition.y}px, rgba(228, 239, 231, 0.4), transparent 40%)`
        }}
      />
      <div className="fixed top-[10%] left-[10%] w-72 h-72 bg-[#E4EFE7] opacity-30 blur-3xl z-0 rounded-full"></div>
      <div className="fixed bottom-[10%] right-[10%] w-72 h-72 bg-[#99BC85] opacity-20 blur-3xl z-0 rounded-full"></div>
    </>
  );
};