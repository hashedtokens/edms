import React from 'react';
import { Link } from 'react-router-dom';

export const ViewAllCard = ({ 
  title = "Endpoints Overview", 
  description = "Browse all available endpoints",
  buttonText = "View All Endpoints",
  to = "/list" 
}) => {
  return (
    <div className="view-all-card bg-gradient-to-br from-[#99BC85] to-[#84A942] text-white rounded-2xl p-8 shadow-lg transition-all duration-300 hover:-translate-y-1 hover:shadow-xl h-48 flex flex-col justify-center items-center text-center">
      <h3 className="text-xl font-semibold mb-2">{title}</h3>
      <p className="text-base mb-6 opacity-90">{description}</p>
      <Link 
        to={to} 
        className="bg-white text-[#2d5016] px-8 py-3 rounded-full font-semibold transition-all duration-300 hover:-translate-y-1 hover:shadow-lg"
      >
        {buttonText}
      </Link>
    </div>
  );
};