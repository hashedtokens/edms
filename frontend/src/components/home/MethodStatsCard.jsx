import React from 'react';
import { MethodBadge } from '../common/MethodBadge';

export const MethodStatsCard = ({ stats }) => {
  const methodStats = stats || [
    { method: 'GET', count: 42 },
    { method: 'POST', count: 18 },
    { method: 'PUT', count: 9 },
    { method: 'DELETE', count: 6 }
  ];

  return (
    <div className="method-card bg-white/80 backdrop-blur-sm rounded-2xl p-8 shadow-lg border border-white/20 transition-all duration-300 hover:-translate-y-1 hover:shadow-xl h-48 flex flex-col justify-center">
      <div className="method-grid grid grid-cols-2 gap-4 w-full">
        {methodStats.map((item) => (
          <div key={item.method} className={`method-item flex justify-between items-center px-4 py-3 rounded-xl ${
            item.method === 'GET' ? 'bg-green-50 border-green-300' :
            item.method === 'POST' ? 'bg-blue-50 border-blue-300' :
            item.method === 'PUT' ? 'bg-orange-50 border-orange-300' :
            'bg-red-50 border-red-300'
          }`}>
            <MethodBadge method={item.method} size="sm" />
            <span className="method-count text-lg font-semibold text-[#2d5016]">
              {item.count}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
};