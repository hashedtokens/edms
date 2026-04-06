import React from 'react';
import { HistoryItem } from './HistoryItem';
import { EndpointItem } from './EndpointItem';

export const TestSidebar = ({
  showBookmarks,
  showEndpoints,
  setShowBookmarks,
  setShowEndpoints,
  bookmarks,
  endpoints,
  history,
  methodFilter,
  setMethodFilter,
  searchQuery,
  setSearchQuery,
  dateSearch,
  setDateSearch,
  loadEndpoints,
  isLoadingEndpoints,
  loadTime,
  toggleBookmark,
  loadFromHistory,
  loadEndpointData,
  filterItems
}) => {
  const currentItems = showBookmarks ? bookmarks : (showEndpoints ? endpoints : history);
  const filteredItems = filterItems(currentItems);

  return (
    <div className="flex flex-col w-96 min-w-96 h-[calc(100vh-80px)] overflow-hidden bg-white border-r border-gray-300 shadow-sm">
      <div className="flex border-b-2 border-gray-300 bg-gray-50">
        <button 
          className={`flex-1 px-4 py-3 bg-transparent border-none text-gray-600 text-sm font-semibold transition-all uppercase ${
            !showBookmarks && !showEndpoints ? 'text-blue-600 border-b-3 border-blue-600 bg-blue-50' : ''
          }`}
          onClick={() => {
            setShowBookmarks(false);
            setShowEndpoints(false);
          }}
        >
          History
        </button>
        <button 
          className={`flex-1 px-4 py-3 bg-transparent border-none text-gray-600 text-sm font-semibold transition-all uppercase ${
            showBookmarks ? 'text-blue-600 border-b-3 border-blue-600 bg-blue-50' : ''
          }`}
          onClick={() => {
            setShowBookmarks(true);
            setShowEndpoints(false);
          }}
        >
          Bookmarks ({bookmarks.length})
        </button>
        <button 
          className={`flex-1 px-4 py-3 bg-transparent border-none text-gray-600 text-sm font-semibold transition-all uppercase ${
            showEndpoints ? 'text-blue-600 border-b-3 border-blue-600 bg-blue-50' : ''
          }`}
          onClick={() => {
            setShowBookmarks(false);
            setShowEndpoints(true);
          }}
        >
          Endpoints ({endpoints.length})
        </button>
      </div>

      <div className="p-4 border-b border-gray-300 bg-gray-50 flex flex-col gap-3">
        <select 
          className="w-full px-3 py-2 border border-gray-300 rounded bg-white text-gray-700 text-sm outline-none hover:border-gray-400"
          value={methodFilter}
          onChange={(e) => setMethodFilter(e.target.value)}
        >
          <option value="ALL">All Methods</option>
          <option value="POST">POST</option>
          <option value="GET">GET</option>
          <option value="PUT">PUT</option>
          <option value="DELETE">DELETE</option>
        </select>
        <input
          type="text"
          placeholder="Search by tags or URL..."
          className="w-full px-3 py-2 border border-gray-300 rounded text-sm outline-none hover:border-gray-400"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
        <input
          type="date"
          className="w-full px-3 py-2 border border-gray-300 rounded text-sm outline-none hover:border-gray-400"
          value={dateSearch}
          onChange={(e) => setDateSearch(e.target.value)}
          title="Filter by date"
        />
        
        <button 
          className="w-full px-4 py-3 bg-gradient-to-r from-purple-600 to-pink-600 text-white border-none rounded font-semibold cursor-pointer text-sm uppercase hover:opacity-90 disabled:opacity-50"
          onClick={loadEndpoints}
          disabled={isLoadingEndpoints}
        >
          {isLoadingEndpoints ? 'Loading...' : '📥 Load 10000 Endpoints'}
        </button>

        {loadTime && (
          <div className="p-3 bg-white border border-gray-300 rounded text-xs text-gray-700 text-center shadow-sm">
            <div className="font-semibold text-purple-700">Total: {endpoints.length} endpoints</div>
            <div>Server: {loadTime.server}ms</div>
            <div>Client: {loadTime.client}ms</div>
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {filteredItems.length === 0 ? (
          <p className="text-gray-500 text-center py-10 italic">
            {currentItems.length === 0 
              ? `No ${showBookmarks ? 'bookmarks' : showEndpoints ? 'endpoints' : 'history'} yet`
              : 'No results found'}
          </p>
        ) : (
          filteredItems.map((item, index) => (
            showEndpoints ? (
              <EndpointItem
                key={index}
                endpoint={item}
                onClick={() => loadEndpointData(item)}
              />
            ) : (
              <HistoryItem
                key={index}
                item={item}
                isBookmarked={item.isBookmarked}
                onToggleBookmark={() => toggleBookmark(item.id)}
                onClick={() => loadFromHistory(item)}
              />
            )
          ))
        )}
      </div>
    </div>
  );
};