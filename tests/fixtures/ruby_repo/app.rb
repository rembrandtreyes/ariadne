require 'json'
require_relative 'helpers'

module Authentication
  class User
    def initialize(name)
      @name = name
    end

    def authenticate(password)
      validate_password(password)
      generate_token
    end

    def self.find_by_email(email)
      # lookup logic
    end

    private

    def validate_password(password)
      raise "Invalid" if password.nil?
    end

    def generate_token
      "token_#{@name}"
    end
  end
end

def main
  user = Authentication::User.new("alice")
  user.authenticate("secret")
end
