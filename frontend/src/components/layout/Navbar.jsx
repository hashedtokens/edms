import React from 'react';
import { Link, useLocation } from 'react-router-dom';

export const Navbar = () => {
  const location = useLocation();
  const activePage = location.pathname === '/' ? 'home' : 
                     location.pathname.includes('testview') ? 'testview' : 'list';

  return (
    <nav className="fixed w-full bg-[rgba(193,211,183,0.95)] z-50 py-2 transition-all duration-300">
      <div className="max-w-7xl mx-auto px-8 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10">
            <img src="/ht-logo.png" alt="HashedTokens Logo" className="w-full h-full object-contain" />
          </div>
          <h1 className="text-2xl font-bold text-[#2d5016] whitespace-nowrap">
            HashedTokens
          </h1>
        </div>
        
        <div className="flex items-center gap-4">
          <Link 
            to="/" 
            className={`px-5 py-2 rounded-full font-semibold transition-all duration-300 ${
              activePage === 'home' 
                ? 'bg-[#4a90e2] text-white shadow-lg' 
                : 'bg-[#99BC85] text-white hover:bg-[#84A942] hover:-translate-y-0.5 hover:shadow-md'
            }`}
          >
            Home
          </Link>
          <Link 
            to="/testview" 
            className={`px-5 py-2 rounded-full font-semibold transition-all duration-300 ${
              activePage === 'testview' 
                ? 'bg-[#4a90e2] text-white shadow-lg' 
                : 'bg-[#99BC85] text-white hover:bg-[#84A942] hover:-translate-y-0.5 hover:shadow-md'
            }`}
          >
            Test view
          </Link>
          <Link 
            to="/list" 
            className={`px-5 py-2 rounded-full font-semibold transition-all duration-300 ${
              activePage === 'list' 
                ? 'bg-[#4a90e2] text-white shadow-lg' 
                : 'bg-[#99BC85] text-white hover:bg-[#84A942] hover:-translate-y-0.5 hover:shadow-md'
            }`}
          >
            List view
          </Link>
        </div>
      </div>
    </nav>
  );
};