import React from 'react';

export const SearchBar = ({ 
  searchQuery, 
  setSearchQuery, 
  searchFilter, 
  setSearchFilter, 
  isSearchApplied, 
  applySearch, 
  clearSearch,
  placeholder = "Search..."
}) => {
  return (
    <div className="search-bar p-4 bg-gray-50 border-b border-gray-200">
      <div className="flex items-center gap-3 max-w-3xl">
        <select 
          value={searchFilter}
          onChange={(e) => setSearchFilter(e.target.value)}
          className="px-3 py-2 border border-gray-300 rounded bg-white text-black text-sm outline-none hover:border-gray-400 focus:border-blue-500"
        >
          <option value="all">All Fields</option>
          <option value="method">Method</option>
          <option value="url">URL</option>
          <option value="tags">Tags</option>
          <option value="annotation">Annotation</option>
          <option value="date">Date & Time</option>
        </select>
        <input 
          type="text"
          placeholder={placeholder}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          onKeyPress={(e) => e.key === 'Enter' && applySearch()}
          className="flex-1 px-3 py-2 border border-gray-300 rounded text-black text-sm outline-none hover:border-gray-400 focus:border-blue-500 focus:ring-2 focus:ring-blue-200"
        />
        <button 
          className="px-4 py-2 bg-blue-600 text-white border-none rounded cursor-pointer text-sm hover:bg-blue-700 disabled:bg-gray-500 disabled:cursor-not-allowed"
          onClick={applySearch}
          disabled={!searchQuery.trim()}
        >
          Apply
        </button>
        {isSearchApplied && (
          <button 
            className="px-4 py-2 bg-gray-600 text-white border-none rounded cursor-pointer text-sm hover:bg-gray-700"
            onClick={clearSearch}
          >
            Clear
          </button>
        )}
      </div>
    </div>
  );
};