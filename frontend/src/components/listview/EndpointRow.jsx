import React from 'react';
import { MethodBadge } from '../common/MethodBadge';

export const EndpointRow = ({
  endpoint,
  isSelected,
  isExpanded,
  onToggleSelect,
  onToggleTags,
  onClick,
  getTotalRequestSize,
  getTotalResponseSize
}) => {
  return (
    <div 
      className={`grid grid-cols-[50px,100px,250px,150px,280px,180px] px-4 py-4 border-b border-gray-200 transition-all hover:bg-gray-50 ${
        isSelected ? 'bg-blue-50' : ''
      }`}
    >
      <div className="flex items-center justify-center" onClick={(e) => e.stopPropagation()}>
        <input 
          type="checkbox" 
          checked={isSelected}
          onChange={(e) => {
            e.stopPropagation();
            onToggleSelect();
          }}
          className="cursor-pointer w-4 h-4"
        />
      </div>
      <div className="flex items-center cursor-pointer" onClick={onClick}>
        <MethodBadge method={endpoint.method} size="sm" />
      </div>
      <div className="flex items-center font-mono text-sm cursor-pointer" onClick={onClick}>
        {endpoint.url}
      </div>
      <div className="flex flex-col items-start">
        <div className="flex items-center gap-2">
          <button 
            className="bg-transparent border-none text-base cursor-pointer p-1 opacity-70 hover:opacity-100 hover:scale-110 transition-all"
            onClick={(e) => {
              e.stopPropagation();
              onToggleTags();
            }}
            title="View tags"
          >
            🏷️
          </button>
          <span className="text-xs text-gray-600 font-semibold bg-gray-100 px-2 py-1 rounded-full">
            {endpoint.tags.length}
          </span>
        </div>
        {isExpanded && (
          <div className="flex flex-wrap gap-1 mt-2 pt-2 border-t border-gray-200 w-full">
            {endpoint.tags.map((tag, idx) => (
              <span key={idx} className="px-2 py-1 bg-indigo-100 text-indigo-700 rounded text-xs font-medium">
                {tag}
              </span>
            ))}
          </div>
        )}
      </div>
      <div className="flex items-center cursor-pointer" onClick={onClick}>
        <div className="flex flex-col text-xs">
          <div className="flex justify-between gap-1">
            <span className="text-gray-600 font-medium">Req:</span>
            <span className="text-gray-800 font-mono">{getTotalRequestSize(endpoint)}</span>
          </div>
          <div className="flex justify-between gap-1">
            <span className="text-gray-600 font-medium">Res:</span>
            <span className="text-gray-800 font-mono">{getTotalResponseSize(endpoint)}</span>
          </div>
        </div>
      </div>
      <div className="flex items-center cursor-pointer" onClick={onClick}>
        <div className="w-full max-h-[70px] overflow-y-auto p-2 bg-white border border-gray-300 rounded text-xs text-gray-700">
          {endpoint.annotation}
        </div>
      </div>
      <div className="flex items-center text-xs text-gray-600 cursor-pointer" onClick={onClick}>
        {endpoint.date} • {endpoint.time}
      </div>
    </div>
  );
};