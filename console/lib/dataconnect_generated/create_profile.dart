part of 'generated.dart';

class CreateProfileVariablesBuilder {
  String name;
  Optional<String> _metadata = Optional.optional(nativeFromJson, nativeToJson);

  final FirebaseDataConnect _dataConnect;  CreateProfileVariablesBuilder metadata(String? t) {
   _metadata.value = t;
   return this;
  }

  CreateProfileVariablesBuilder(this._dataConnect, {required  this.name,});
  Deserializer<CreateProfileData> dataDeserializer = (dynamic json)  => CreateProfileData.fromJson(jsonDecode(json));
  Serializer<CreateProfileVariables> varsSerializer = (CreateProfileVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<CreateProfileData, CreateProfileVariables>> execute() {
    return ref().execute();
  }

  MutationRef<CreateProfileData, CreateProfileVariables> ref() {
    CreateProfileVariables vars= CreateProfileVariables(name: name,metadata: _metadata,);
    return _dataConnect.mutation("CreateProfile", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class CreateProfileProfileInsert {
  final String id;
  CreateProfileProfileInsert.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateProfileProfileInsert otherTyped = other as CreateProfileProfileInsert;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  CreateProfileProfileInsert({
    required this.id,
  });
}

@immutable
class CreateProfileData {
  final CreateProfileProfileInsert profile_insert;
  CreateProfileData.fromJson(dynamic json):
  
  profile_insert = CreateProfileProfileInsert.fromJson(json['profile_insert']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateProfileData otherTyped = other as CreateProfileData;
    return profile_insert == otherTyped.profile_insert;
    
  }
  @override
  int get hashCode => profile_insert.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['profile_insert'] = profile_insert.toJson();
    return json;
  }

  CreateProfileData({
    required this.profile_insert,
  });
}

@immutable
class CreateProfileVariables {
  final String name;
  late final Optional<String>metadata;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  CreateProfileVariables.fromJson(Map<String, dynamic> json):
  
  name = nativeFromJson<String>(json['name']) {
  
  
  
    metadata = Optional.optional(nativeFromJson, nativeToJson);
    metadata.value = json['metadata'] == null ? null : nativeFromJson<String>(json['metadata']);
  
  }
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateProfileVariables otherTyped = other as CreateProfileVariables;
    return name == otherTyped.name && 
    metadata == otherTyped.metadata;
    
  }
  @override
  int get hashCode => Object.hashAll([name.hashCode, metadata.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['name'] = nativeToJson<String>(name);
    if(metadata.state == OptionalState.set) {
      json['metadata'] = metadata.toJson();
    }
    return json;
  }

  CreateProfileVariables({
    required this.name,
    required this.metadata,
  });
}

