import React from 'react';

export const FilterBar = ({ filterType, setFilterType, filters = ['all', 'tags', 'annotations', 'date'] }) => {
  return (
    <div className="p-4 bg-white border-b border-gray-200 flex gap-3">
      {filters.map((filter) => (
        <button 
          key={filter}
          onClick={() => setFilterType(filter)}
          className={`px-4 py-2 bg-white border border-gray-300 rounded cursor-pointer capitalize text-sm transition-all ${
            filterType === filter 
              ? 'bg-blue-600 text-white border-blue-600' 
              : 'hover:bg-gray-50'
          }`}
        >
          {filter}
        </button>
      ))}
    </div>
  );
};