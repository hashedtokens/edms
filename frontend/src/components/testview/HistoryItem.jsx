import React, { useState } from 'react';
import { getMethodColor } from '../../utils/getMethodColor';

export const HistoryItem = ({ item, isBookmarked, onToggleBookmark, onClick }) => {
  const [expandedTags, setExpandedTags] = useState(false);

  const toggleTags = (e) => {
    e.stopPropagation();
    setExpandedTags(!expandedTags);
  };

  return (
    <div 
      className={`mb-3 p-4 bg-white rounded-lg border border-gray-300 cursor-pointer transition-all hover:bg-gray-50 hover:border-purple-500 ${
        isBookmarked ? 'border-l-4 border-yellow-500' : 'border-l-4 border-transparent'
      }`}
      onClick={onClick}
    >
      <div className="flex justify-between items-center mb-3">
        <span 
          className="px-3 py-1 rounded text-xs font-bold text-white uppercase"
          style={{ backgroundColor: getMethodColor(item.method) }}
        >
          {item.method}
        </span>
        <div className="flex gap-2 items-center">
          {item.tags && item.tags.length > 0 && (
            <div className="flex items-center gap-1">
              <button 
                className="bg-transparent border-none text-base cursor-pointer p-1 opacity-70 hover:opacity-100"
                onClick={toggleTags}
                title="View tags"
              >
                🏷️
              </button>
              <span className="text-xs text-gray-600 font-semibold bg-gray-100 px-2 rounded">
                {item.tags.length}
              </span>
            </div>
          )}
          <label className="cursor-pointer" onClick={(e) => e.stopPropagation()}>
            <input
              type="checkbox"
              className="hidden"
              checked={isBookmarked}
              onChange={onToggleBookmark}
            />
            <div className={`w-5 h-5 border-2 rounded flex items-center justify-center ${
              isBookmarked 
                ? 'bg-purple-600 border-purple-600' 
                : 'border-gray-300'
            }`}>
              {isBookmarked && (
                <span className="text-white text-xs">✓</span>
              )}
            </div>
          </label>
        </div>
      </div>
      <div className="text-sm text-gray-800 font-medium mb-2 truncate hover:text-purple-600">
        {item.url}
      </div>
      <div className="flex justify-between items-center pt-2 border-t border-gray-100">
        <span className="text-xs text-gray-500 italic">
          {new Date(item.timestamp).toLocaleDateString()}
        </span>
      </div>
      {expandedTags && item.tags && item.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-3 pt-3 border-t border-gray-200">
          {item.tags.map((tag, i) => (
            <span key={i} className="px-2 py-1 bg-purple-100 text-purple-700 border border-purple-200 rounded text-xs">
              {tag}
            </span>
          ))}
        </div>
      )}
      {item.annotation && (
        <div className="text-xs text-gray-500 mt-2 truncate">
          {item.annotation}
        </div>
      )}
    </div>
  );
};