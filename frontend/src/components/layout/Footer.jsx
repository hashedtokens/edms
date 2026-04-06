import React from 'react';

export const Footer = () => {
  return (
    <footer className="border-t-2 border-[#E4EFE7] py-8 bg-[#FAF1E6] mt-auto">
      <div className="max-w-7xl mx-auto px-8 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9">
            <img src="/ht-logo.png" alt="HashedTokens Logo" className="w-full h-full object-contain" />
          </div>
          <span className="font-semibold text-[#2d5016] text-lg">
            HashedTokens
          </span>
        </div>
        <p className="text-[#5a5a5a] text-sm">
          © 2025 HashedTokens. All rights reserved.
        </p>
      </div>
    </footer>
  );
};