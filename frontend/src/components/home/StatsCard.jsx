import React from 'react';

export const StatsCard = ({ number, label, icon, color = '#2d5016' }) => {
  return (
    <div className="stat-card bg-white/80 backdrop-blur-sm rounded-2xl p-8 shadow-lg border border-white/20 transition-all duration-300 hover:-translate-y-1 hover:shadow-xl h-48 flex flex-col justify-center items-center text-center">
      {icon && <div className="mb-2 text-2xl">{icon}</div>}
      <h3 className="text-5xl font-bold mb-2" style={{ color }}>
        {number}
      </h3>
      <p className="text-lg text-[#5a5a5a] font-medium">
        {label}
      </p>
    </div>
  );
};